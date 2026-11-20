// The Android app's Crypto.kt and Pairing.kt, line for line in Java, to check the desktop
// app reads and writes exactly the same bytes. Run by `cargo test -- --ignored interop`.
import java.io.*;
import java.nio.ByteBuffer;
import java.nio.file.*;
import java.security.*;
import java.security.spec.*;
import java.util.*;
import java.util.zip.*;
import javax.crypto.*;
import javax.crypto.spec.*;

public class Phone {
    static final byte[] MAGIC = "MEMI1".getBytes();

    static byte[] seal(byte[] key, byte[] plain) throws Exception {
        ByteArrayOutputStream bo = new ByteArrayOutputStream();
        try (GZIPOutputStream gz = new GZIPOutputStream(bo)) { gz.write(plain); }
        byte[] iv = new byte[12]; new SecureRandom().nextBytes(iv);
        Cipher c = Cipher.getInstance("AES/GCM/NoPadding");
        c.init(Cipher.ENCRYPT_MODE, new SecretKeySpec(key, "AES"), new GCMParameterSpec(128, iv));
        c.updateAAD(MAGIC);
        byte[] sealed = c.doFinal(bo.toByteArray());
        ByteBuffer out = ByteBuffer.allocate(5 + 16 + 12 + sealed.length);
        out.put(MAGIC).put(new byte[16]).put(iv).put(sealed);
        return out.array();
    }

    static byte[] open(byte[] key, byte[] blob) throws Exception {
        byte[] iv = Arrays.copyOfRange(blob, 21, 33);
        Cipher c = Cipher.getInstance("AES/GCM/NoPadding");
        c.init(Cipher.DECRYPT_MODE, new SecretKeySpec(key, "AES"), new GCMParameterSpec(128, iv));
        c.updateAAD(MAGIC);
        byte[] packed = c.doFinal(blob, 33, blob.length - 33);
        return new GZIPInputStream(new ByteArrayInputStream(packed)).readAllBytes();
    }

    static byte[] sealFor(String code, byte[] syncKey) throws Exception {
        String rest = code.substring("memi:pair:1:".length());
        String id = rest.substring(0, rest.indexOf(':'));
        byte[] pub = Base64.getUrlDecoder().decode(rest.substring(rest.indexOf(':') + 1));
        PublicKey computer = KeyFactory.getInstance("EC").generatePublic(new X509EncodedKeySpec(pub));
        KeyPairGenerator g = KeyPairGenerator.getInstance("EC"); g.initialize(new ECGenParameterSpec("secp256r1"));
        KeyPair mine = g.generateKeyPair();
        KeyAgreement ka = KeyAgreement.getInstance("ECDH"); ka.init(mine.getPrivate()); ka.doPhase(computer, true);
        byte[] ikm = ka.generateSecret();
        Mac m1 = Mac.getInstance("HmacSHA256"); m1.init(new SecretKeySpec(new byte[32], "HmacSHA256"));
        byte[] prk = m1.doFinal(ikm);
        Mac m2 = Mac.getInstance("HmacSHA256"); m2.init(new SecretKeySpec(prk, "HmacSHA256"));
        byte[] info = ("memi-pair-v1:" + id).getBytes();
        byte[] infoPlus = Arrays.copyOf(info, info.length + 1); infoPlus[info.length] = 1;
        byte[] secret = m2.doFinal(infoPlus);
        byte[] iv = new byte[12]; new SecureRandom().nextBytes(iv);
        Cipher c = Cipher.getInstance("AES/GCM/NoPadding");
        c.init(Cipher.ENCRYPT_MODE, new SecretKeySpec(secret, "AES"), new GCMParameterSpec(128, iv));
        byte[] sealed = c.doFinal(syncKey);
        byte[] p = mine.getPublic().getEncoded();
        return ByteBuffer.allocate(2 + p.length + 12 + sealed.length).putShort((short) p.length).put(p).put(iv).put(sealed).array();
    }

    public static void main(String[] a) throws Exception {
        Path dir = Paths.get(a[0]);
        byte[] key = Files.readAllBytes(dir.resolve("key.bin"));
        // 1. Phone seals a backup; desktop must open it.
        Files.write(dir.resolve("from-phone.bin"), seal(key, Files.readAllBytes(dir.resolve("backup.json"))));
        // 2. Desktop sealed one; phone must open it.
        Files.write(dir.resolve("desktop-opened.json"), open(key, Files.readAllBytes(dir.resolve("from-desktop.bin"))));
        // 3. Phone answers the QR code with the key.
        String code = new String(Files.readAllBytes(dir.resolve("code.txt"))).trim();
        Files.write(dir.resolve("pair.bin"), sealFor(code, key));
    }
}
