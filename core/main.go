// Optional helper stub — not required for MVP.
// Future: embed sing-box as a library and speak JSON-RPC to the GUI.
package main

import (
	"fmt"
	"os"
)

func main() {
	fmt.Fprintln(os.Stderr, "singbox-core helper: not used in process-based MVP")
	fmt.Fprintln(os.Stderr, "Run official sing-box binary instead.")
	os.Exit(0)
}
