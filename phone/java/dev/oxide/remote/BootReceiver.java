package dev.oxide.remote;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;

/** After a restart: the watch connection comes back by itself when it was on. */
public class BootReceiver extends BroadcastReceiver {
    @Override
    public void onReceive(Context c, Intent i) {
        if (c.getSharedPreferences("bridge", Context.MODE_PRIVATE).getBoolean("on", false)) {
            try {
                Intent s = new Intent(c, BridgeService.class);
                if (android.os.Build.VERSION.SDK_INT >= 26) c.startForegroundService(s);
                else c.startService(s);
            } catch (Exception ignored) {
            }
        }
    }
}
