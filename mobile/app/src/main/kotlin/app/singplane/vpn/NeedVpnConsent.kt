package app.singplane.vpn

import android.content.Intent

class NeedVpnConsent(val consentIntent: Intent) : Exception("需要系统 VPN 授权")
