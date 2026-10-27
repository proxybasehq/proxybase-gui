# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.
#
# For more details, see
#   http://developer.android.com/guide/developing/tools/proguard.html

# JNI entry points. These classes and methods are looked up by name from
# Rust (src-tauri/src/android_vpn_jni.rs), so R8 cannot see any callers and
# would strip or rename them in release builds:
#   - VpnController.startVpn / stopVpn      -> NoSuchMethodError at tap time
#   - ProxyBaseVpnService.protectSocketFromRust -> relay sockets not protected
# Native methods themselves must also keep their exact names so the
# Java_xyz_proxybase_desktop_* symbols in libproxybase_gui_lib.so resolve.
-keep class xyz.proxybase.desktop.VpnController { *; }
-keep class xyz.proxybase.desktop.ProxyBaseVpnService { *; }

# If your project uses WebView with JS, uncomment the following
# and specify the fully qualified class name to the JavaScript interface
# class:
#-keepclassmembers class fqcn.of.javascript.interface.for.webview {
#   public *;
#}

# Uncomment this to preserve the line number information for
# debugging stack traces.
#-keepattributes SourceFile,LineNumberTable

# If you keep the line number information, uncomment this to
# hide the original source file name.
#-renamesourcefileattribute SourceFile