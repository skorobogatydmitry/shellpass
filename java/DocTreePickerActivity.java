package java;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;

public class DocTreePickerActivity extends Activity {

    // make the override for nativeOnActivityResult visible
    static {
        System.loadLibrary("shellpass");
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
        startActivityForResult(intent, 1);
    }

    @Override
    protected void onActivityResult(
        int requestCode,
        int resultCode,
        Intent data
    ) {
        super.onActivityResult(requestCode, resultCode, data);
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
