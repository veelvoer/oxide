package dev.oxide.remote;

import android.content.Context;
import android.hardware.Sensor;
import android.hardware.SensorEvent;
import android.hardware.SensorEventListener;
import android.hardware.SensorManager;
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
    private float[] prevAcc = null;
    private long lastSpike = 0;
    private int spikes = 0;
    private int ready = 0;

    @Override
    protected void onCreate(Bundle b) {
        super.onCreate(b);
        sm = (SensorManager) getSystemService(Context.SENSOR_SERVICE);
    }

    /** Switch the sensors on or off. level: how hard a pinch must tap (lower = more sensitive). */
    public synchronized void motion(boolean enable, boolean pinch, float level) {
        pinchOn = pinch;
        pinchLevel = level;
        if (enable == on || sm == null) return;
        on = enable;
        if (enable) {
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
        float[] r = { yaw, pitch, ready };
        yaw = pitch = 0;
        ready = 0;
        return r;
    }

    @Override
    public synchronized void onSensorChanged(SensorEvent e) {
        if (e.sensor.getType() == Sensor.TYPE_GYROSCOPE) {
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
                if (jerk > pinchLevel && gyroMag < 1.2f && (now - lastSpike) > 90_000_000L) {
                    lastSpike = now;
                    spikes++;
                }
            }
            if (prevAcc == null) prevAcc = new float[3];
            System.arraycopy(v, 0, prevAcc, 0, 3);
        }
    }

    @Override
    public void onAccuracyChanged(Sensor s, int accuracy) {}

    @Override
    protected void onPause() {
        if (on) motion(false, pinchOn, pinchLevel);
        super.onPause();
    }
}
