package dev.oxide.remote;

import android.app.NativeActivity;
import android.app.PendingIntent;
import android.content.Intent;
import android.content.pm.ActivityInfo;
import android.content.pm.PackageInstaller;
import android.content.res.Configuration;
import android.graphics.Color;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.provider.Settings;
import android.view.HapticFeedbackConstants;
import android.view.View;
import android.view.Window;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.view.WindowManager;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;

/**
 * The app is written in Rust (a NativeActivity); this class only does what has to happen on Android's UI thread or
 * through Java APIs: system bar insets, the system colors (Material You), full screen, haptics, small HTTP requests
 * and installing an update. The Rust side calls these methods through JNI and polls the results.
 */
public class OxideActivity extends NativeActivity {
    // system bars and keyboard, in pixels: top, bottom, left, right, keyboard
    private volatile int[] insets = new int[5];
    // update: 0 idle, 1 downloading, 2 waiting for the user, 3 failed; progress 0..100
    private static volatile int updateState = 0;
    private static volatile int updateProgress = 0;
    private static volatile String updateError = "";

    static void updateFailed(String why) {
        updateError = why;
        updateState = 3;
    }

    static void updateInstalled() {
        updateState = 4;
    }

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        Window w = getWindow();
        // draw behind the system bars; the app keeps its content out of them with the insets
        w.setStatusBarColor(Color.TRANSPARENT);
        w.setNavigationBarColor(Color.TRANSPARENT);
        if (Build.VERSION.SDK_INT >= 29) {
            w.setNavigationBarContrastEnforced(false);
            w.setStatusBarContrastEnforced(false);
        }
        if (Build.VERSION.SDK_INT >= 30) {
            w.setDecorFitsSystemWindows(false);
        } else {
            w.getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_LAYOUT_STABLE | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN);
        }
        if (Build.VERSION.SDK_INT >= 28) {
            w.getAttributes().layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
        }
        w.getDecorView().setOnApplyWindowInsetsListener((v, in) -> {
            int[] n = new int[5];
            if (Build.VERSION.SDK_INT >= 30) {
                android.graphics.Insets bars = in.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
                android.graphics.Insets ime = in.getInsets(WindowInsets.Type.ime());
                n[0] = bars.top; n[1] = bars.bottom; n[2] = bars.left; n[3] = bars.right; n[4] = ime.bottom;
            } else {
                n[0] = in.getSystemWindowInsetTop(); n[1] = in.getSystemWindowInsetBottom();
                n[2] = in.getSystemWindowInsetLeft(); n[3] = in.getSystemWindowInsetRight();
            }
            insets = n;
            return v.onApplyWindowInsets(in);
        });
        lightBars(!isDark());
        handleInstallIntent(getIntent());
        if (getSharedPreferences("bridge", MODE_PRIVATE).getBoolean("on", false)) applyBridge();
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        handleInstallIntent(intent);
    }

    // ------------------------------------------------------------------ for Rust

    public int[] getInsets() {
        return insets;
    }

    /** The phone's name as the user set it, else its model. */
    public String deviceName() {
        String n = null;
        try {
            n = Settings.Global.getString(getContentResolver(), Settings.Global.DEVICE_NAME);
        } catch (Exception ignored) {
        }
        if (n == null || n.isEmpty()) n = Build.MODEL;
        return n == null ? "" : n;
    }

    /** May this app switch wireless debugging on? (granted once from the PC: oxide-ctl phone setup) */
    public boolean canShowOnPc() {
        return checkSelfPermission("android.permission.WRITE_SECURE_SETTINGS") == android.content.pm.PackageManager.PERMISSION_GRANTED;
    }

    /** Wireless debugging on or off: with it on, the PC can show this phone's screen (scrcpy) over Wi-Fi. */
    public boolean setShowOnPc(boolean on) {
        try {
            Settings.Global.putInt(getContentResolver(), "adb_wifi_enabled", on ? 1 : 0);
            return true;
        } catch (Exception e) {
            return false;
        }
    }

    public boolean isDark() {
        return (getResources().getConfiguration().uiMode & Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES;
    }

    /** Material You colors (Android 12+): accent1 tones 100 200 500 600 800, neutral1 10 50 100 800 900, accent2 100 700. Empty before 12. */
    public int[] systemColors() {
        if (Build.VERSION.SDK_INT < 31) return new int[0];
        int[] ids = {
            android.R.color.system_accent1_100, android.R.color.system_accent1_200, android.R.color.system_accent1_500,
            android.R.color.system_accent1_600, android.R.color.system_accent1_800,
            android.R.color.system_neutral1_10, android.R.color.system_neutral1_50, android.R.color.system_neutral1_100,
            android.R.color.system_neutral1_800, android.R.color.system_neutral1_900,
            android.R.color.system_accent2_100, android.R.color.system_accent2_700,
            android.R.color.system_neutral2_200, android.R.color.system_neutral2_700,
        };
        int[] out = new int[ids.length];
        for (int i = 0; i < ids.length; i++) out[i] = getResources().getColor(ids[i], getTheme());
        return out;
    }

    /** Dark icons on light bars (and the other way round). */
    public void lightBars(boolean light) {
        runOnUiThread(() -> {
            if (Build.VERSION.SDK_INT >= 30) {
                WindowInsetsController c = getWindow().getInsetsController();
                if (c == null) return;
                int mask = WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS | WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS;
                c.setSystemBarsAppearance(light ? mask : 0, mask);
            } else {
                View d = getWindow().getDecorView();
                int f = d.getSystemUiVisibility();
                f = light ? (f | View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR | View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR)
                          : (f & ~View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR & ~View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR);
                d.setSystemUiVisibility(f);
            }
        });
    }

    /** Full screen: no system bars (a swipe from the edge shows them for a moment), landscape, screen stays on. */
    public void setFullscreen(boolean on) {
        runOnUiThread(() -> {
            Window w = getWindow();
            if (Build.VERSION.SDK_INT >= 30) {
                WindowInsetsController c = w.getInsetsController();
                if (c != null) {
                    if (on) {
                        c.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
                        c.hide(WindowInsets.Type.systemBars());
                    } else {
                        c.show(WindowInsets.Type.systemBars());
                    }
                }
            } else {
                w.getDecorView().setSystemUiVisibility(on
                    ? (View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY | View.SYSTEM_UI_FLAG_FULLSCREEN | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                       | View.SYSTEM_UI_FLAG_LAYOUT_STABLE | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN)
                    : (View.SYSTEM_UI_FLAG_LAYOUT_STABLE | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN));
            }
            setRequestedOrientation(on ? ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE : ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED);
        });
    }

    public void keepScreenOn(boolean on) {
        runOnUiThread(() -> {
            if (on) getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
            else getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        });
    }

    public void haptic(int kind) {
        runOnUiThread(() -> {
            // 0 tap, 1 long press, 2 confirm
            int c = HapticFeedbackConstants.VIRTUAL_KEY;
            if (kind == 1) c = HapticFeedbackConstants.LONG_PRESS;
            if (kind == 2 && Build.VERSION.SDK_INT >= 30) c = HapticFeedbackConstants.CONFIRM;
            getWindow().getDecorView().performHapticFeedback(c);
        });
    }

    public void openUrl(String url) {
        runOnUiThread(() -> {
            try {
                startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            } catch (Exception ignored) {
            }
        });
    }

    /** A small GET (blocking; call it from a background thread). Empty on any error. */
    public String httpGet(String url) {
        try {
            HttpURLConnection c = (HttpURLConnection) new URL(url).openConnection();
            c.setConnectTimeout(8000);
            c.setReadTimeout(10000);
            c.setRequestProperty("User-Agent", "OxideRemote");
            c.setRequestProperty("Accept", "application/json");
            if (c.getResponseCode() / 100 != 2) return "";
            try (InputStream in = c.getInputStream()) {
                ByteArrayOutputStream out = new ByteArrayOutputStream();
                byte[] b = new byte[16384];
                int n;
                while ((n = in.read(b)) > 0 && out.size() < 4 * 1024 * 1024) out.write(b, 0, n);
                return out.toString("UTF-8");
            }
        } catch (Exception e) {
            return "";
        }
    }

    public int[] updateStatus() {
        return new int[] { updateState, updateProgress };
    }

    public String updateError() {
        return updateError;
    }

    /** Download the APK at `url` and hand it to the system installer (it asks the user to confirm). */
    public void installUpdate(String url) {
        if (updateState == 1) return;
        if (Build.VERSION.SDK_INT >= 26 && !getPackageManager().canRequestPackageInstalls()) {
            // the user allows this app to install updates once, then presses Update again
            runOnUiThread(() -> {
                try {
                    startActivity(new Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:" + getPackageName())));
                } catch (Exception ignored) {
                }
            });
            updateError = "Allow Oxide Remote to install updates, then press Update again.";
            updateState = 3;
            return;
        }
        updateState = 1;
        updateProgress = 0;
        updateError = "";
        new Thread(() -> {
            try {
                File apk = new File(getCacheDir(), "update.apk");
                HttpURLConnection c = (HttpURLConnection) new URL(url).openConnection();
                c.setInstanceFollowRedirects(true);
                c.setConnectTimeout(10000);
                c.setReadTimeout(20000);
                c.setRequestProperty("User-Agent", "OxideRemote");
                long total = c.getContentLengthLong();
                try (InputStream in = c.getInputStream(); OutputStream out = new FileOutputStream(apk)) {
                    byte[] b = new byte[65536];
                    long got = 0;
                    int n;
                    while ((n = in.read(b)) > 0) {
                        out.write(b, 0, n);
                        got += n;
                        if (total > 0) updateProgress = (int) (got * 100 / total);
                    }
                }
                PackageInstaller pi = getPackageManager().getPackageInstaller();
                PackageInstaller.SessionParams p = new PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL);
                p.setAppPackageName(getPackageName());
                if (Build.VERSION.SDK_INT >= 31) p.setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED);
                int id = pi.createSession(p);
                try (PackageInstaller.Session s = pi.openSession(id)) {
                    try (OutputStream out = s.openWrite("base.apk", 0, apk.length()); InputStream in = new FileInputStream(apk)) {
                        byte[] b = new byte[65536];
                        int n;
                        while ((n = in.read(b)) > 0) out.write(b, 0, n);
                        s.fsync(out);
                    }
                    Intent back = new Intent(this, InstallReceiver.class).setPackage(getPackageName());
                    int flags = PendingIntent.FLAG_UPDATE_CURRENT | (Build.VERSION.SDK_INT >= 31 ? PendingIntent.FLAG_MUTABLE : 0);
                    PendingIntent pend = PendingIntent.getBroadcast(this, 7, back, flags);
                    s.commit(pend.getIntentSender());
                }
                updateState = 2; // handed to Android; the installer answers through InstallReceiver
            } catch (Exception e) {
                updateError = String.valueOf(e.getMessage());
                updateState = 3;
            }
        }).start();
    }

    /**
     * The watch connection: 0 off, 1 on, 2 waiting for the Bluetooth permission (the system asks), 3 no Bluetooth.
     * With `enable` null-less: call watchBridge(true/false) to switch, watchBridgeState() to look.
     */
    public int watchBridge(boolean enable) {
        android.content.SharedPreferences p = getSharedPreferences("bridge", MODE_PRIVATE);
        p.edit().putBoolean("on", enable).apply();
        return applyBridge();
    }

    public int watchBridgeState() {
        boolean on = getSharedPreferences("bridge", MODE_PRIVATE).getBoolean("on", false);
        if (!on) return 0;
        if (android.bluetooth.BluetoothAdapter.getDefaultAdapter() == null) return 3;
        if (Build.VERSION.SDK_INT >= 31 && checkSelfPermission("android.permission.BLUETOOTH_CONNECT") != android.content.pm.PackageManager.PERMISSION_GRANTED) return 2;
        return 1;
    }

    private int applyBridge() {
        boolean on = getSharedPreferences("bridge", MODE_PRIVATE).getBoolean("on", false);
        Intent svc = new Intent(this, BridgeService.class);
        if (!on) {
            stopService(svc);
            return 0;
        }
        if (android.bluetooth.BluetoothAdapter.getDefaultAdapter() == null) return 3;
        java.util.ArrayList<String> need = new java.util.ArrayList<>();
        if (Build.VERSION.SDK_INT >= 31 && checkSelfPermission("android.permission.BLUETOOTH_CONNECT") != android.content.pm.PackageManager.PERMISSION_GRANTED) need.add("android.permission.BLUETOOTH_CONNECT");
        if (Build.VERSION.SDK_INT >= 33 && checkSelfPermission("android.permission.POST_NOTIFICATIONS") != android.content.pm.PackageManager.PERMISSION_GRANTED) need.add("android.permission.POST_NOTIFICATIONS");
        if (!need.isEmpty()) {
            final String[] perms = need.toArray(new String[0]);
            runOnUiThread(() -> requestPermissions(perms, 77));
        }
        if (need.contains("android.permission.BLUETOOTH_CONNECT")) return 2;
        try {
            if (Build.VERSION.SDK_INT >= 26) startForegroundService(svc);
            else startService(svc);
        } catch (Exception e) {
            return 2;
        }
        return 1;
    }

    @Override
    public void onRequestPermissionsResult(int code, String[] perms, int[] results) {
        super.onRequestPermissionsResult(code, perms, results);
        if (code == 77) applyBridge();
    }

    private void handleInstallIntent(Intent intent) {
        if (intent == null || !"dev.oxide.remote.INSTALL".equals(intent.getAction())) return;
        int status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, -999);
        if (status == PackageInstaller.STATUS_PENDING_USER_ACTION) {
            Intent confirm = intent.getParcelableExtra(Intent.EXTRA_INTENT);
            if (confirm != null) startActivity(confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
        } else if (status != PackageInstaller.STATUS_SUCCESS && status != -999) {
            String m = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE);
            updateError = m == null ? "The update was not installed." : m;
            updateState = 3;
        }
    }
}
