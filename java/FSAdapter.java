package java;

import android.content.ContentResolver;
import android.content.Context;
import android.database.Cursor;
import android.net.Uri;
import android.provider.DocumentsContract;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.util.ArrayList;

public class FSAdapter {

    public static String[] listFilesRecursive(Context context, String uriStr) {
        Uri uri = Uri.parse(uriStr);
        ArrayList<String> results = new ArrayList<>();
        ArrayList<String> toTraverse = new ArrayList<>();
        toTraverse.add(DocumentsContract.getTreeDocumentId(uri));
        while (!toTraverse.isEmpty()) {
            String currentDirId = toTraverse.remove(toTraverse.size() - 1);
            Uri childrenUri = DocumentsContract.buildChildDocumentsUriUsingTree(
                uri,
                currentDirId
            );
            ContentResolver resolver = context.getContentResolver();
            Cursor cursor = resolver.query(
                childrenUri,
                new String[] {
                    DocumentsContract.Document.COLUMN_DOCUMENT_ID,
                    DocumentsContract.Document.COLUMN_MIME_TYPE,
                },
                null,
                null,
                null
            );

            if (cursor == null) continue;
            try {
                while (cursor.moveToNext()) {
                    String childDocId = cursor.getString(0);
                    String mimeType = cursor.getString(1);
                    Uri childUri = DocumentsContract.buildDocumentUriUsingTree(
                        uri,
                        childDocId
                    );
                    if (
                        DocumentsContract.Document.MIME_TYPE_DIR.equals(
                            mimeType
                        )
                    ) {
                        toTraverse.add(childDocId);
                    } else {
                        results.add(childUri.toString());
                    }
                }
            } finally {
                cursor.close();
            }
        }
        return results.toArray(new String[0]);
    }

    public static byte[] readFile(Context context, String uriStr)
        throws IOException {
        Uri uri = Uri.parse(uriStr);
        ContentResolver resolver = context.getContentResolver();
        InputStream is = resolver.openInputStream(uri);
        if (is == null) throw new IOException("Could not open input stream");

        ByteArrayOutputStream buffer = new ByteArrayOutputStream();
        byte[] chunk = new byte[8192];
        int n;
        try {
            while ((n = is.read(chunk)) != -1) {
                buffer.write(chunk, 0, n);
            }
        } finally {
            is.close();
        }
        return buffer.toByteArray();
    }
}
