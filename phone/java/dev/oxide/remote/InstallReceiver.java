package dev.oxide.remote;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageInstaller;

/** The installer's answer to an update session. Works without the activity (it is gone while the app is replaced). */
public class InstallReceiver extends BroadcastReceiver {
    @Override
    public void onReceive(Context c, Intent i) {
        int status = i.getIntExtra(PackageInstaller.EXTRA_STATUS, -999);
        if (status == PackageInstaller.STATUS_PENDING_USER_ACTION) {
            // Android wants one tap from the user ("Update"): show its screen
            Intent confirm = i.getParcelableExtra(Intent.EXTRA_INTENT);
            if (confirm != null) {
                confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
                try {
                    c.startActivity(confirm);
                } catch (Exception e) {
                    OxideActivity.updateFailed("Android did not open its install screen: " + e.getMessage());
                }
            }
        } else if (status == PackageInstaller.STATUS_SUCCESS) {
            OxideActivity.updateInstalled();
        } else if (status != -999) {
            String m = i.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE);
            OxideActivity.updateFailed(m == null ? "The update was not installed." : "The update was not installed: " + m);
        }
    }
}
