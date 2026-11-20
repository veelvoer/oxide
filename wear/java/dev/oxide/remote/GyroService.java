package dev.oxide.remote;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.os.Build;
import android.os.IBinder;
import android.os.PowerManager;

/**
 * Keeps the app's process (and with it the gyro mouse's sensors and the Bluetooth link) alive while the watch's screen is
 * off. Runs only while a PC is open with the gyro mouse on; the app stops it when the PC is left or the app is closed.
 */
public class GyroService extends Service {
    private PowerManager.WakeLock lock;

    @Override
    public IBinder onBind(Intent i) {
        return null;
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        NotificationManager nm = (NotificationManager) getSystemService(Context.NOTIFICATION_SERVICE);
        if (Build.VERSION.SDK_INT >= 26) {
            nm.createNotificationChannel(new NotificationChannel("gyro", "Gyro mouse", NotificationManager.IMPORTANCE_LOW));
        }
        PendingIntent open = PendingIntent.getActivity(this, 0, new Intent(this, WatchActivity.class), PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        Notification.Builder b = Build.VERSION.SDK_INT >= 26 ? new Notification.Builder(this, "gyro") : new Notification.Builder(this);
        b.setContentTitle("Gyro mouse is on").setContentText("Tap to open Oxide Remote").setSmallIcon(android.R.drawable.ic_menu_compass).setContentIntent(open).setOngoing(true);
        if (Build.VERSION.SDK_INT >= 29) startForeground(43, b.build(), ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE);
        else startForeground(43, b.build());
        if (lock == null) {
            PowerManager pm = (PowerManager) getSystemService(Context.POWER_SERVICE);
            lock = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "oxide:gyro");
            lock.setReferenceCounted(false);
        }
        if (!lock.isHeld()) lock.acquire(4 * 60 * 60 * 1000L);
        return START_NOT_STICKY;
    }

    @Override
    public void onDestroy() {
        if (lock != null && lock.isHeld()) lock.release();
        super.onDestroy();
    }
}
