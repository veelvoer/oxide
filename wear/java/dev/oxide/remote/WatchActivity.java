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
            Sensor rv = abs ? sm.getDefaultSensor(Sensor.TYPE_GAME_ROTATION_VECTOR) : null;
            if (rv != null) sm.registerListener(this, rv, SensorManager.SENSOR_DELAY_GAME);
            Sensor g = sm.getDefaultSensor(Sensor.TYPE_GYROSCOPE);
            Sensor a = sm.getDefaultSensor(Sensor.TYPE_ACCELEROMETER);
            // SENSOR_DELAY_GAME is about 50 Hz: smooth enough for a pointer, light on the battery
            if (g != null) sm.registerListener(this, g, SensorManager.SENSOR_DELAY_GAME);
            if (a != null) sm.registerListener(this, a, SensorManager.SENSOR_DELAY_FASTEST);
            yaw = pitch = 0;
            lastGyro = 0;
            prevAcc = null;
            spikes = 0;
            ready = 0;
        } else {
            sm.unregisterListener(this);
        }
    }

    public boolean hasGyro() {
        return sm != null && sm.getDefaultSensor(Sensor.TYPE_GYROSCOPE) != null;
    }

    /** { yaw, pitch, pinches } since the last call. */
    public synchronized float[] motionPoll() {
        long now = System.nanoTime();
        if (spikes > 0 && (now - lastSpike) > 350_000_000L) {
            ready = spikes;
            spikes = 0;
        }
        long ago = spikeAt == 0 ? -1 : (now - spikeAt) / 1_000_000L;
        float[] r = { yaw, pitch, ready, armYaw, armPitch, armOk ? 1f : 0f, ago >= 0 && ago < 2000 ? (float) ago : -1f };
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
                // a pinch is sharp but the wrist is not moving much
                if (jerk > pinchLevel && gyroMag < (absolute ? 2.4f : 1.2f) && (now - lastSpike) > 90_000_000L) {
                    lastSpike = now;
                    spikeAt = now;
                    spikes++;
                }
            }
            if (prevAcc == null) prevAcc = new float[3];
            System.arraycopy(v, 0, prevAcc, 0, 3);
        }
    }

    @Override
    public void onAccuracyChanged(Sensor s, int accuracy) {}

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
