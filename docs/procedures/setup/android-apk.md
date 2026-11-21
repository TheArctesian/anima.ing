# Android APK and F-Droid build

This procedure serves M5-7 of the [mobile plan](../../plans/mobile-plan.md). It builds an unsigned APK for `ing.anima.mobile` on x86-64 Linux.

## Release from GitHub

The [Android release workflow](../../../.github/workflows/android-release.yml) builds from a push that advances `main` by one first-parent commit. It uses the shared release version and the first-parent count as the Android version code. GitHub uploads `Anima.apk` to a draft release, then publishes the release with a tag of the form `android-0.8.21+821`. The Android workflow has its own switch, `ANIMA_PUBLIC_ANDROID=on`. Once the run passes, use the `fdroid-submission` artifact for the first catalog submission.

Before the first release, create the GitHub environment `release-android` and restrict it to the branch `main`. Set the two environment secrets below with the existing Android release key. Set the environment variable to the certificate's SHA-256 in lowercase hexadecimal. The value in the environment must match `AllowedAPKSigningKeys` in the F-Droid recipe. The key alias in the keystore is `anima-android-release`.

| Setting | Value |
| --- | --- |
| Secret `ANDROID_RELEASE_KEYSTORE_BASE64` | Base64 of the existing PKCS#12 release keystore |
| Secret `ANDROID_RELEASE_KEYSTORE_PASSWORD` | Password for the release keystore |
| Variable `ANDROID_RELEASE_CERTIFICATE_SHA256` | SHA-256 of the public signing certificate |

Set the keystore secret from Terminal with `base64 -i "$RELEASE_KEYSTORE" | gh secret set ANDROID_RELEASE_KEYSTORE_BASE64 --env release-android`. Set the password through GitHub's environment settings or the interactive prompt of `gh secret set`. Only the signing job reads the secrets.

## Build on Linux

1. Use a fresh Git checkout of the commit to build. Install Git, a C compiler, `make`, `pkg-config`, and JDK 21.
2. Install Bun 1.3.14 and Go 1.27.4. Keep both on `PATH`. The repository lockfiles pin the application dependencies and Tauri CLI 2.12.4.
3. Install Rust through `rustup`. Run `rustup target add aarch64-linux-android` in the checkout. The repository pins the compiler version in [`rust-toolchain.toml`](../../../rust-toolchain.toml).
4. Set `JAVA_HOME` to JDK 21. Set `ANDROID_HOME` to the Android SDK. Tauri compiles the app against SDK 37.0. The keyboard plugin compiles against SDK 36. Use `sdkmanager` to install both.

   ```sh
   sdkmanager --sdk_root="$ANDROID_HOME" \
     'platforms;android-36' 'platforms;android-37.0' 'build-tools;36.0.0' 'ndk;28.2.13676358'
   ```

5. Pass the version name and Android version code for the release. The following values are for a local build check.

   ```sh
   apps/desktop/src-tauri/build-android-apk.sh 0.1.0 1000
   ```

   Expected output file is `target/android/Anima-unsigned.apk`. Check the package and the two native libraries.

   ```sh
   "$ANDROID_HOME/build-tools/36.0.0/aapt" dump badging target/android/Anima-unsigned.apk
   unzip -Z1 target/android/Anima-unsigned.apk | grep '^lib/arm64-v8a/'
   ```

   Expected package is `ing.anima.mobile` at the chosen version. The archive contains `libanima_app_lib.so` and `libphonelink.so` for `arm64-v8a`.

## Prepare the F-Droid submission

Use the draft [F-Droid recipe](../../../apps/desktop/fdroid/ing.anima.mobile.yml). The inputs are the release commit, version, upstream APK URL and signing certificate fingerprint. The draft build stays disabled until these inputs are available and the release meets the phone checks in the mobile plan.

The upstream listing reads the [Fastlane metadata](../../../fastlane/metadata/android/en-US/) from the release tag. Before submission, check the Android release against [HT-NEW-mobilesession](../../need-human-testing.md#ht-new-mobilesession-keep-and-clear-a-mobile-session-m5-1).

The recipe reads both version fields from Android release tags and downloads the APK from the matching GitHub release. Once the first build is accepted, F-Droid's update checker can add later releases from their tags. F-Droid publishes the upstream-signed APK after its build verifies against that APK.

The checkout includes the [project license](../../../LICENSE), [font notices](../../../apps/client/src/styles/fonts/) and the [Carbon icon license](../../../apps/client/src/lib/icons/LICENSE-Carbon.txt). The F-Droid source scanner flagged a Go test fixture. The recipe removes it from the recipe checkout because the production build does not use it.

1. Copy the recipe to `metadata/ing.anima.mobile.yml` in an `fdroiddata` checkout. Set `commit`, `versionName` and `versionCode` to the first release.
2. Use the signed asset from the matching GitHub release. For a manual build, sign the upstream APK with the Android release keystore. Use build-tools 36.0.0 and let `apksigner` prompt for the passwords. `--alignment-preserved` keeps the APK layout that the [signature comparison tool](https://github.com/obfusk/apksigcopier#what-about-signatures-made-by-apksigner-from-build-tools--3500-rc1) expects.

   ```sh
   "$ANDROID_HOME/build-tools/36.0.0/apksigner" sign --alignment-preserved \
     --ks "$RELEASE_KEYSTORE" --ks-key-alias "$RELEASE_KEY_ALIAS" \
     --in target/android/Anima-unsigned.apk --out target/android/Anima.apk
   "$ANDROID_HOME/build-tools/36.0.0/apksigner" verify --print-certs target/android/Anima.apk
   ```

   Check that `Binaries` resolves to the versioned URL of that APK. Check that `AllowedAPKSigningKeys` holds the signing certificate SHA-256.
3. Remove `disable` from the build entry. Run `fdroid lint ing.anima.mobile` and `fdroid build --server ing.anima.mobile` using the F-Droid build server. Review the scanner output and the dependency licenses.
4. Compare the signed APK to the unsigned APK from that build. Put the Android build-tools folder on `PATH` so the comparison can run `apksigner`.

   ```sh
   apksigcopier compare --unsigned /path/to/signed-release.apk /path/to/fdroid-rebuild.apk
   ```

   Expected result is exit code zero. Keep the same release signing key for both channels. F-Droid uses the APK signed by Anima when its rebuild matches. Submit the metadata after the checks pass.

See the [F-Droid submission guide](https://f-droid.org/docs/Submitting_to_F-Droid_Quick_Start_Guide/) and the [reproducible builds guide](https://f-droid.org/docs/Reproducible_Builds/).
