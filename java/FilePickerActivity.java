package java;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;

public class FilePickerActivity extends Activity {

    // make the override for nativeOnActivityResult visible
    static {
        System.loadLibrary("shellpass");
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.setType("*/*");
        startActivityForResult(intent, 1);
    }

    @Override
    protected void onActivityResult(
        int requestCode,
        int resultCode,
        Intent data
    ) {
        super.onActivityResult(requestCode, resultCode, data);
        if (resultCode == RESULT_OK && data != null) {
            Uri uri = data.getData();
            if (uri != null) {
                final int takeFlags =
                    data.getFlags() & (Intent.FLAG_GRANT_READ_URI_PERMISSION);
                getContentResolver().takePersistableUriPermission(
                    uri,
                    takeFlags
                );
            }
        }
        nativeOnActivityResult(
            requestCode,
            resultCode,
            data.getData().toString()
        );
        finish();
    }

    public native void nativeOnActivityResult(
        int requestCode,
        int resultCode,
        String data
    );
}
