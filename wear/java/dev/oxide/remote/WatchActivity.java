package dev.oxide.remote;

import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothManager;
import android.bluetooth.BluetoothSocket;
import android.content.Context;
import android.content.pm.PackageManager;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.UUID;
import android.hardware.Sensor;
import android.hardware.SensorEvent;
import android.hardware.SensorEventListener;
import android.hardware.SensorManager;
import android.os.Build;
import android.os.Bundle;

/**
 * The watch's own part on top of the phone app's activity: the gyro mouse and the pinch detector. Sensors are only
 * switched on while the gyro pad is on screen (the battery of a watch is small). The Rust side polls: it takes the
 * movement since the last poll and the number of pinches that just ended.
 */
public class WatchActivity extends OxideActivity implements SensorEventListener {
    private SensorManager sm;
    private boolean on = false;
    private boolean pinchOn = true;
    private float pinchLevel = 1.6f;

    // wrist movement since the last poll (radians): around the axis out of the screen (hand left/right) and around the
    // axis to the right (hand up/down)
    private float yaw = 0, pitch = 0;
    private long lastGyro = 0;
    private float gyroMag = 0;

    // pinch: a short sharp tap on the accelerometer while the wrist is steady. Taps that follow each other within
    // 350 ms count together (1 = click, 3 = right click).
    private volatile boolean keepAlive = false;
    private long spikeAt = 0;
    private boolean absolute = false;
    private float armSign = 1f;
    private float armYaw = 0, armPitch = 0;
    private boolean armOk = false;
    private final float[] rot = new float[9];

    // the pinch detector adapts to the noise of the moment: a tap must stand out from it
    private float noise = 0.06f;
    private float threshold = 0.5f;
    private float jerkPeak = 0;
    private int newSpikes = 0;
    private float[] prevAcc = null;
    private long lastSpike = 0;
    private int spikes = 0;
    private int ready = 0;

    @Override
    protected void onCreate(Bundle b) {
        super.onCreate(b);
        sm = (SensorManager) getSystemService(Context.SENSOR_SERVICE);
        if (Build.VERSION.SDK_INT >= 31 && checkSelfPermission("android.permission.BLUETOOTH_CONNECT") != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(new String[] { "android.permission.BLUETOOTH_CONNECT" }, 78);
        }
    }

    // ---- the phone, over Bluetooth: the watch needs no Wi-Fi. The phone's Oxide Remote app listens on a service; the
    // conversation is in Rust (link.rs), this holds the socket.
    private static final String SERVICE_UUID = "5e0a6f1c-6b0d-4c0f-9d7c-0f3a4b2e9a11";
    private volatile BluetoothSocket sock = null;
    private volatile String btError = "";

    public String btError() {
        return btError;
    }

    /** 0 ready, 1 no permission, 2 Bluetooth is off, 3 no paired devices, 4 no Bluetooth */
    public int btState() {
        try {
            if (Build.VERSION.SDK_INT >= 31 && checkSelfPermission("android.permission.BLUETOOTH_CONNECT") != PackageManager.PERMISSION_GRANTED) return 1;
            BluetoothManager bm = (BluetoothManager) getSystemService(Context.BLUETOOTH_SERVICE);
            BluetoothAdapter a = bm == null ? null : bm.getAdapter();
            if (a == null) return 4;
            if (!a.isEnabled()) return 2;
            if (a.getBondedDevices().isEmpty()) return 3;
            return 0;
        } catch (SecurityException e) {
            return 1;
        }
    }

    /** Connect to the phone: the address that worked last time first, then the other paired devices. "" = not found. */
    public synchronized String btConnect(String last) {
        btClose();
        if (btState() != 0) return "";
        try {
            BluetoothAdapter a = ((BluetoothManager) getSystemService(Context.BLUETOOTH_SERVICE)).getAdapter();
            java.util.ArrayList<BluetoothDevice> list = new java.util.ArrayList<>();
            for (BluetoothDevice d : a.getBondedDevices()) {
                if (d.getAddress().equals(last)) list.add(0, d);
                else list.add(d);
            }
            for (BluetoothDevice d : list) {
                try {
                    BluetoothSocket s = d.createRfcommSocketToServiceRecord(UUID.fromString(SERVICE_UUID));
                    s.connect();
                    sock = s;
                    return d.getAddress();
                } catch (Exception e) {
                    btError = d.getName() + ": " + e.getMessage();
                    android.util.Log.w("oxide-watch", "bluetooth connect failed: " + btError);
                }
            }
        } catch (SecurityException e) {
            btError = "no permission";
        }
        return "";
    }

    public InputStream btIn() throws java.io.IOException {
        return sock.getInputStream();
    }

    public OutputStream btOut() throws java.io.IOException {
        return sock.getOutputStream();
    }

    public synchronized void btClose() {
        BluetoothSocket s = sock;
        sock = null;
        if (s != null) {
            try {
                s.close();
            } catch (Exception ignored) {
            }
        }
    }

    /** Switch the sensors on or off. level: how hard a pinch must tap (lower = more sensitive). */
    public synchronized void motion(boolean enable, boolean pinch, float level, boolean abs, float sign) {
        pinchOn = pinch;
        pinchLevel = level;
        armSign = sign;
        if (enable && on && abs != absolute) {
            // the mode changed: start the sensors again
            sm.unregisterListener(this);
            on = false;
        }
        absolute = abs;
        if (enable == on || sm == null) return;
        on = enable;
        if (enable) {
            armOk = false;
            // wake-up sensors keep delivering while the watch sleeps (battery saver, screen off); the plain ones are the fallback
            Sensor rv = abs ? sensor(Sensor.TYPE_GAME_ROTATION_VECTOR) : null;
            if (rv != null) sm.registerListener(this, rv, 10_000, 0);
            Sensor g = sensor(Sensor.TYPE_GYROSCOPE);
            Sensor a = sensor(Sensor.TYPE_ACCELEROMETER);
            // about 100 Hz: smooth enough for a pointer
            if (g != null) sm.registerListener(this, g, 10_000, 0);
            if (a != null) sm.registerListener(this, a, SensorManager.SENSOR_DELAY_FASTEST, 0);
            yaw = pitch = 0;
            lastGyro = 0;
            prevAcc = null;
            spikes = 0;
            ready = 0;
        } else {
            sm.unregisterListener(this);
        }
    }

    private Sensor sensor(int type) {
        Sensor s = null;
        try {
            s = sm.getDefaultSensor(type, true);
        } catch (Exception ignored) {
        }
        return s != null ? s : sm.getDefaultSensor(type);
    }

    /** True when Android lets this app run in the background without limits (battery saver and Samsung's sleeping apps). */
    public boolean batteryExempt() {
        android.os.PowerManager pm = (android.os.PowerManager) getSystemService(Context.POWER_SERVICE);
        return pm != null && pm.isIgnoringBatteryOptimizations(getPackageName());
    }

    /** Ask for that: the system shows its own question; where it has none, the app's page in the settings opens. */
    public void requestBatteryExempt() {
        runOnUiThread(() -> {
            try {
                startActivity(new android.content.Intent(android.provider.Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, android.net.Uri.parse("package:" + getPackageName())));
            } catch (Exception e) {
                try {
                    startActivity(new android.content.Intent(android.provider.Settings.ACTION_APPLICATION_DETAILS_SETTINGS, android.net.Uri.parse("package:" + getPackageName())));
                } catch (Exception ignored) {
                }
            }
        });
    }

    public boolean hasGyro() {
        return sm != null && sm.getDefaultSensor(Sensor.TYPE_GYROSCOPE) != null;
    }

    /** { yaw, pitch, pinches } since the last call. */
    public synchronized float[] motionPoll() {
        long now = System.nanoTime();
        if (spikes > 0 && (now - lastSpike) > 300_000_000L) {
            ready = spikes;
            spikes = 0;
        }
        long ago = spikeAt == 0 ? -1 : (now - spikeAt) / 1_000_000L;
        float[] r = { yaw, pitch, ready, armYaw, armPitch, armOk ? 1f : 0f, ago >= 0 && ago < 2000 ? (float) ago : -1f, newSpikes, jerkPeak, threshold };
        newSpikes = 0;
        jerkPeak = 0;
        yaw = pitch = 0;
        ready = 0;
        return r;
    }

    @Override
    public synchronized void onSensorChanged(SensorEvent e) {
        if (e.sensor.getType() == Sensor.TYPE_GAME_ROTATION_VECTOR) {
            // where the arm points: the watch's x axis (toward the hand on the left wrist) in the world
            SensorManager.getRotationMatrixFromVector(rot, e.values);
            float vx = armSign * rot[0], vy = armSign * rot[3], vz = armSign * rot[6];
            armYaw = (float) Math.atan2(vx, vy);
            armPitch = (float) Math.asin(Math.max(-1f, Math.min(1f, vz)));
            armOk = true;
        } else if (e.sensor.getType() == Sensor.TYPE_GYROSCOPE) {
            long t = e.timestamp;
            if (lastGyro != 0) {
                float dt = (t - lastGyro) / 1e9f;
                if (dt > 0 && dt < 0.2f) {
                    yaw += e.values[2] * dt;
                    pitch += e.values[0] * dt;
                }
            }
            lastGyro = t;
            gyroMag = (float) Math.sqrt(e.values[0] * e.values[0] + e.values[1] * e.values[1] + e.values[2] * e.values[2]);
        } else if (e.sensor.getType() == Sensor.TYPE_ACCELEROMETER && pinchOn) {
            float[] v = e.values;
            if (prevAcc != null) {
                float dx = v[0] - prevAcc[0], dy = v[1] - prevAcc[1], dz = v[2] - prevAcc[2];
                float jerk = (float) Math.sqrt(dx * dx + dy * dy + dz * dz);
                long now = e.timestamp;
                // pinchLevel = how many times louder than the noise a tap must be (lower = more sensitive); never below 0.3 m/s2
                threshold = Math.max(0.3f, pinchLevel * noise);
                if (jerk > jerkPeak) jerkPeak = jerk;
                // a pinch is sharp; the wrist may be moving (pointing) but not turning fast
                if (jerk > threshold && gyroMag < (absolute ? 3.0f : 1.6f) && (now - lastSpike) > 110_000_000L) {
                    lastSpike = now;
                    spikeAt = now;
                    spikes++;
                    newSpikes++;
                } else if (jerk <= threshold) {
                    noise = noise * 0.985f + jerk * 0.015f;
                }
            }
            if (prevAcc == null) prevAcc = new float[3];
            System.arraycopy(v, 0, prevAcc, 0, 3);
        }
    }

    @Override
    public void onAccuracyChanged(Sensor s, int accuracy) {}

    // ---- the watch's own controls: the touch bezel scrolls, the back button goes back (the app decides what back means)
    private static native void nativeInput(int kind, float value);

    private float rotaryAcc = 0;

    @Override
    public boolean dispatchGenericMotionEvent(android.view.MotionEvent e) {
        if (e.getAction() == android.view.MotionEvent.ACTION_SCROLL && e.isFromSource(android.view.InputDevice.SOURCE_ROTARY_ENCODER)) {
            // clockwise is positive scroll in the app; a few pixels per click, as the system's lists
            float v = -e.getAxisValue(android.view.MotionEvent.AXIS_SCROLL) * android.view.ViewConfiguration.get(this).getScaledVerticalScrollFactor();
            try {
                nativeInput(0, v);
            } catch (UnsatisfiedLinkError ignored) {
            }
            rotaryAcc += Math.abs(v);
            if (rotaryAcc > 24f) {
                rotaryAcc = 0;
                getWindow().getDecorView().performHapticFeedback(android.view.HapticFeedbackConstants.CLOCK_TICK);
            }
            return true;
        }
        return super.dispatchGenericMotionEvent(e);
    }

    @Override
    public boolean dispatchKeyEvent(android.view.KeyEvent e) {
        if (e.getKeyCode() == android.view.KeyEvent.KEYCODE_BACK) {
            if (e.getAction() == android.view.KeyEvent.ACTION_UP && !e.isCanceled()) {
                try {
                    nativeInput(1, 0f);
                } catch (UnsatisfiedLinkError ignored) {
                    finish();
                }
            }
            return true;
        }
        return super.dispatchKeyEvent(e);
    }

    @Override
    public void haptic(int kind) {
        if (kind == 3) clockTick();
        else super.haptic(kind);
    }

    /** The tick of a watch's crown or bezel. */
    public void clockTick() {
        runOnUiThread(() -> getWindow().getDecorView().performHapticFeedback(android.view.HapticFeedbackConstants.CLOCK_TICK));
    }

    /** The screen's brightness for this window: 0 = as dark as it goes, negative = the system's own. */
    public void screenBrightness(float level) {
        runOnUiThread(() -> {
            android.view.WindowManager.LayoutParams lp = getWindow().getAttributes();
            lp.screenBrightness = level < 0 ? android.view.WindowManager.LayoutParams.BRIGHTNESS_OVERRIDE_NONE : Math.max(0f, level);
            getWindow().setAttributes(lp);
        });
    }

    /** The app has nothing further back to go to: leave (the watch face is next). */
    public void exitApp() {
        runOnUiThread(this::finish);
    }

    /** While the pointer is used the screen may turn off: a foreground service keeps the process and the sensors going. */
    public void keepAlive(boolean enable) {
        keepAlive = enable;
        android.content.Intent svc = new android.content.Intent(this, GyroService.class);
        try {
            if (enable) {
                if (Build.VERSION.SDK_INT >= 26) startForegroundService(svc);
                else startService(svc);
            } else {
                stopService(svc);
            }
        } catch (Exception e) {
            android.util.Log.w("oxide-watch", "gyro service: " + e);
        }
    }

    public boolean motionActive() {
        return on;
    }

    @Override
    protected void onPause() {
        // with the service the sensors stay on while the screen is off; without it they stop
        if (on && !keepAlive) motion(false, pinchOn, pinchLevel, absolute, armSign);
        super.onPause();
    }

    @Override
    protected void onDestroy() {
        // the app is closed: nothing keeps running
        keepAlive = false;
        if (on) motion(false, pinchOn, pinchLevel, absolute, armSign);
        try {
            stopService(new android.content.Intent(this, GyroService.class));
        } catch (Exception ignored) {
        }
        btClose();
        super.onDestroy();
    }
}
