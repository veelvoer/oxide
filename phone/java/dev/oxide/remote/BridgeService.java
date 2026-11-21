package dev.oxide.remote;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothManager;
import android.bluetooth.BluetoothServerSocket;
import android.bluetooth.BluetoothSocket;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.os.Build;
import android.os.IBinder;

import java.io.InputStream;
import java.io.OutputStream;
import java.util.UUID;

/**
 * Lets the user's watch use this phone as its connection to the PCs: a Bluetooth service the watch connects to. The
 * conversation itself is in Rust (`bridge.rs`); this only accepts connections and hands over the two streams. Runs as a
 * foreground service (a quiet notification) so Android keeps it alive while the app is closed.
 */
public class BridgeService extends Service {
    static {
        System.loadLibrary("oxide_remote");
    }

    static final String SERVICE_UUID = "5e0a6f1c-6b0d-4c0f-9d7c-0f3a4b2e9a11";
    private static volatile Thread acceptor = null;
    private static volatile BluetoothServerSocket server = null;

    private static native void serve(InputStream in, OutputStream out, String dir, String phoneName);

    @Override
    public IBinder onBind(Intent i) {
        return null;
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        try {
            startAsForeground();
        } catch (Exception e) {
            // Android did not allow a foreground service now (the system restarted it in the background): not a crash
            android.util.Log.w("oxide-remote", "bridge service: " + e);
            stopSelf();
            return START_NOT_STICKY;
        }
        synchronized (BridgeService.class) {
            if (acceptor == null || !acceptor.isAlive()) {
                acceptor = new Thread(this::acceptLoop, "oxide-bridge-accept");
                acceptor.start();
            }
        }
        return START_STICKY;
    }

    private void startAsForeground() {
        NotificationManager nm = (NotificationManager) getSystemService(Context.NOTIFICATION_SERVICE);
        String channel = "bridge";
        if (Build.VERSION.SDK_INT >= 26) {
            nm.createNotificationChannel(new NotificationChannel(channel, "Watch connection", NotificationManager.IMPORTANCE_MIN));
        }
        Intent open = new Intent(this, OxideActivity.class);
        PendingIntent pi = PendingIntent.getActivity(this, 0, open, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        Notification.Builder b = Build.VERSION.SDK_INT >= 26 ? new Notification.Builder(this, channel) : new Notification.Builder(this);
        b.setContentTitle("Oxide Remote").setContentText("Your watch can reach your PCs through this phone")
                .setSmallIcon(android.R.drawable.stat_sys_data_bluetooth).setContentIntent(pi).setOngoing(true);
        if (Build.VERSION.SDK_INT >= 29) {
            startForeground(41, b.build(), ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE);
        } else {
            startForeground(41, b.build());
        }
    }

    private void acceptLoop() {
        String dir = getFilesDir().getPath();
        String name = android.os.Build.MODEL;
        while (true) {
            try {
                BluetoothManager bm = (BluetoothManager) getSystemService(Context.BLUETOOTH_SERVICE);
                BluetoothAdapter a = bm == null ? null : bm.getAdapter();
                if (a == null || !a.isEnabled()) {
                    Thread.sleep(5000);
                    continue;
                }
                server = a.listenUsingRfcommWithServiceRecord("Oxide Remote", UUID.fromString(SERVICE_UUID));
                while (true) {
                    final BluetoothSocket s = server.accept();
                    new Thread(() -> {
                        try {
                            InputStream in = s.getInputStream();
                            OutputStream out = s.getOutputStream();
                            serve(in, out, dir, name);
                        } catch (Throwable ignored) {
                        } finally {
                            try {
                                s.close();
                            } catch (Exception ignored) {
                            }
                        }
                    }, "oxide-bridge-watch").start();
                }
            } catch (SecurityException e) {
                // no permission (yet): the app asks for it; look again later
                try {
                    Thread.sleep(5000);
                } catch (InterruptedException ie) {
                    return;
                }
            } catch (InterruptedException e) {
                return;
            } catch (Exception e) {
                try {
                    if (server != null) server.close();
                } catch (Exception ignored) {
                }
                try {
                    Thread.sleep(3000);
                } catch (InterruptedException ie) {
                    return;
                }
            }
        }
    }

    @Override
    public void onDestroy() {
        try {
            if (server != null) server.close();
        } catch (Exception ignored) {
        }
        Thread t = acceptor;
        if (t != null) t.interrupt();
        acceptor = null;
        super.onDestroy();
    }
}
