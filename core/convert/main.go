// singpanel-convert: loopback helper converting Clash / URI body → sing-box outbounds.
// Listens only on 127.0.0.1; auth via one-shot Bearer token printed on stdout.
package main

import (
	cryptoRand "crypto/rand"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"regexp"
	"strings"
	"time"

	"github.com/xmdhs/clash2singbox/convert"
	"github.com/xmdhs/clash2singbox/model"
	"github.com/xmdhs/clash2singbox/model/clash"
	"gopkg.in/yaml.v3"
)

type convertRequest struct {
	SubscriptionBody string            `json:"subscriptionBody"`
	Include          string            `json:"include"`
	Exclude          string            `json:"exclude"`
	SingBoxVersion   string            `json:"singBoxVersion"`
	Options          map[string]string `json:"options"`
}

type warningItem struct {
	Node   string `json:"node"`
	Reason string `json:"reason"`
}

type convertResponse struct {
	OK        bool             `json:"ok"`
	Outbounds []map[string]any `json:"outbounds,omitempty"`
	Endpoints []map[string]any `json:"endpoints,omitempty"`
	Warnings  []warningItem    `json:"warnings"`
	Stats     map[string]int   `json:"stats"`
	Error     string           `json:"error,omitempty"`
}

func main() {
	token := randomToken(24)
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		fmt.Fprintf(os.Stderr, "listen: %v\n", err)
		os.Exit(1)
	}
	port := ln.Addr().(*net.TCPAddr).Port
	// Host / desktop reads this line.
	fmt.Printf("READY port=%d token=%s\n", port, token)
	_ = os.Stdout.Sync()

	mux := http.NewServeMux()
	mux.HandleFunc("/healthz", func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte("ok"))
	})
	mux.HandleFunc("/v1/convert", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		auth := r.Header.Get("Authorization")
		if auth != "Bearer "+token {
			http.Error(w, "unauthorized", http.StatusUnauthorized)
			return
		}
		body, err := io.ReadAll(io.LimitReader(r.Body, 16<<20))
		if err != nil {
			writeJSON(w, convertResponse{OK: false, Error: "read body: " + err.Error()})
			return
		}
		var req convertRequest
		if err := json.Unmarshal(body, &req); err != nil {
			writeJSON(w, convertResponse{OK: false, Error: "invalid json: " + err.Error()})
			return
		}
		// Merge options map if present
		if req.Options != nil {
			if v, ok := req.Options["include"]; ok && req.Include == "" {
				req.Include = v
			}
			if v, ok := req.Options["exclude"]; ok && req.Exclude == "" {
				req.Exclude = v
			}
		}
		resp := doConvert(req)
		writeJSON(w, resp)
	})

	srv := &http.Server{
		Handler:           mux,
		ReadHeaderTimeout: 10 * time.Second,
		ReadTimeout:       60 * time.Second,
		WriteTimeout:      60 * time.Second,
	}
	if err := srv.Serve(ln); err != nil && !errors.Is(err, http.ErrServerClosed) {
		fmt.Fprintf(os.Stderr, "serve: %v\n", err)
		os.Exit(1)
	}
}

func doConvert(req convertRequest) convertResponse {
	raw := strings.TrimSpace(req.SubscriptionBody)
	if raw == "" {
		return convertResponse{OK: false, Error: "subscriptionBody empty", Warnings: []warningItem{}, Stats: map[string]int{}}
	}

	c, inputN, parseWarns, err := parseBody(raw)
	if err != nil {
		return convertResponse{
			OK:       false,
			Error:    err.Error(),
			Warnings: parseWarns,
			Stats:    map[string]int{"inputNodes": inputN},
		}
	}

	outs, eps, convErr := convert.Clash2sing(c, model.SINGLATEST)
	// Clash2sing returns partial results + joined errors for skipped nodes
	var warnings []warningItem
	warnings = append(warnings, parseWarns...)
	if convErr != nil {
		// Split multi-error if possible
		for _, part := range strings.Split(convErr.Error(), "\n") {
			part = strings.TrimSpace(part)
			if part == "" {
				continue
			}
			warnings = append(warnings, warningItem{Node: "*", Reason: part})
		}
	}

	// include/exclude by tag
	filtered := make([]map[string]any, 0, len(outs))
	skippedFilter := 0
	for _, o := range outs {
		m, err := structToMap(o)
		if err != nil {
			warnings = append(warnings, warningItem{Node: o.Tag, Reason: err.Error()})
			continue
		}
		tag, _ := m["tag"].(string)
		if !matchFilter(tag, req.Include, req.Exclude) {
			skippedFilter++
			continue
		}
		filtered = append(filtered, m)
	}

	epMaps := make([]map[string]any, 0, len(eps))
	for _, ep := range eps {
		if ep == nil {
			continue
		}
		m, err := structToMap(ep)
		if err != nil {
			warnings = append(warnings, warningItem{Node: ep.Tag, Reason: err.Error()})
			continue
		}
		epMaps = append(epMaps, m)
	}

	converted := len(filtered) + len(epMaps)
	if converted == 0 {
		return convertResponse{
			OK:       false,
			Error:    "converted 0 nodes",
			Warnings: warnings,
			Stats: map[string]int{
				"inputNodes": inputN,
				"converted":  0,
				"skipped":    inputN,
			},
		}
	}

	return convertResponse{
		OK:        true,
		Outbounds: filtered,
		Endpoints: epMaps,
		Warnings:  warnings,
		Stats: map[string]int{
			"inputNodes": inputN,
			"converted":  converted,
			"skipped":    max(0, inputN-converted) + skippedFilter,
		},
	}
}

func parseBody(raw string) (clash.Clash, int, []warningItem, error) {
	var warns []warningItem

	// Try Clash YAML first
	var c clash.Clash
	if err := yaml.Unmarshal([]byte(raw), &c); err == nil && len(c.Proxies) > 0 {
		return c, len(c.Proxies), warns, nil
	}

	// URI list
	lines := strings.Split(raw, "\n")
	var proxies []clash.Proxies
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		// skip pure yaml leftovers
		if !strings.Contains(line, "://") {
			continue
		}
		p, err := convert.ParseURL(line)
		if err != nil {
			warns = append(warns, warningItem{Node: line, Reason: err.Error()})
			continue
		}
		proxies = append(proxies, p)
	}
	if len(proxies) == 0 {
		return clash.Clash{}, 0, warns, fmt.Errorf("not clash yaml and no valid node URIs")
	}
	return clash.Clash{Proxies: proxies}, len(proxies), warns, nil
}

func matchFilter(tag, include, exclude string) bool {
	if include != "" {
		ok, err := matchRegexp(include, tag)
		if err != nil || !ok {
			return false
		}
	}
	if exclude != "" {
		ok, err := matchRegexp(exclude, tag)
		if err == nil && ok {
			return false
		}
	}
	return true
}

func matchRegexp(pattern, s string) (bool, error) {
	r, err := regexp.Compile(pattern)
	if err != nil {
		return false, err
	}
	return r.MatchString(s), nil
}

func structToMap(v any) (map[string]any, error) {
	b, err := json.Marshal(v)
	if err != nil {
		return nil, err
	}
	var m map[string]any
	if err := json.Unmarshal(b, &m); err != nil {
		return nil, err
	}
	// drop empty / null-ish noise keys optional
	return m, nil
}

func writeJSON(w http.ResponseWriter, resp convertResponse) {
	if resp.Warnings == nil {
		resp.Warnings = []warningItem{}
	}
	if resp.Stats == nil {
		resp.Stats = map[string]int{}
	}
	w.Header().Set("Content-Type", "application/json")
	enc := json.NewEncoder(w)
	_ = enc.Encode(resp)
}

func randomToken(n int) string {
	const letters = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
	b := make([]byte, n)
	if _, err := io.ReadFull(cryptoRand.Reader, b); err != nil {
		// Fallback to timestamp + counter if crypto/rand fails
		x := uint64(time.Now().UnixNano())
		for i := range b {
			x = x*6364136223846793005 + 1
			b[i] = letters[int(x>>33)%len(letters)]
		}
		return string(b)
	}
	for i := range b {
		b[i] = letters[int(b[i])%len(letters)]
	}
	return string(b)
}

func max(a, b int) int {
	if a > b {
		return a
	}
	return b
}
