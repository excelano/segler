#!/bin/sh
# Ask an *installed* Segler what it actually is, on the machine it is on.
#
# `build-app.sh --store` checks what it built, on the machine that built it.
# That is a different question from this one, and the gap between them is
# where a submission goes wrong: the artefact is carried to another machine,
# macOS decides something about it there, and nothing in the build says what.
#
# Everything below is a fact a command can settle. What a command cannot
# settle, whether the window is laid out correctly, whether the icons are
# right, what Gatekeeper shows a *person*, is in `CHECKLIST.md` and needs eyes.
#
#     ./check-install.sh                    # /Applications/Segler.app
#     ./check-install.sh /path/to/App       # somewhere else
#
# It reports; it does not repair, and it does not stop at the first bad
# answer, since an hour on a borrowed machine is the wrong place to learn one
# thing per run. slipcase-desktop's script, with the team read out of the
# signature rather than written here.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)
set -u

app="${1:-/Applications/Segler.app}"
bundle_id="com.excelano.segler-desktop"

findings=0
say()  { printf '  %-46s %s\n' "$1" "$2"; }
ok()   { say "$1" "ok — $2"; }
bad()  { say "$1" "NO — $2"; findings=$((findings + 1)); }
note() { printf '  %-46s %s\n' "$1" "$2"; }

echo "check-install.sh on $(uname -m), macOS $(sw_vers -productVersion)"
echo "$app"
echo

[ -d "$app" ] || { echo "  not installed at $app"; exit 2; }

exe="${app}/Contents/MacOS/segler-desktop"

# 1. The architecture question the whole trip is about. A universal binary
#    that lost its arm64 slice to a bad `lipo` would install and run here
#    under Rosetta and look entirely normal while doing it.
arches=$(lipo -info "$exe" 2>/dev/null | sed 's/.*are: //')
case "$arches" in
    *arm64*) ok "the binary has an arm64 slice" "$arches" ;;
    *) bad "the binary has an arm64 slice" "${arches:-lipo could not read it}" ;;
esac

# 2. And whether the machine is actually running that slice, which is not the
#    same claim. Asked of the running process rather than the file: `vmmap`
#    prints the code type of a process you own, and Rosetta is invisible in
#    every other listing. Compared against *this machine*, not against arm64,
#    so it is right on an Intel Mac too.
pid=$(pgrep -f "Segler.app/Contents/MacOS/segler-desktop" | head -1)
if [ -n "$pid" ]; then
    code_type=$(vmmap "$pid" 2>/dev/null | sed -n 's/^Code Type: *//p' | head -1)
    case "$(uname -m),$code_type" in
        arm64,ARM64*|x86_64,X86*) ok "the running process is native" "$code_type on $(uname -m)" ;;
        arm64,X86*) bad "the running process is native" "$code_type on arm64 — under Rosetta" ;;
        *,"") note "the running process is native" "vmmap said nothing usable" ;;
        *) bad "the running process is native" "$code_type on $(uname -m)" ;;
    esac
else
    note "the running process is native" "not running — launch it and re-run"
fi

# 3. Which kind of build this is, decided from the certificate rather than
#    from what the caller believed. They want *different* answers below: a
#    Store build must carry an application identifier and a profile, a
#    Developer ID build must carry neither, and a TestFlight build carries the
#    identifier and no profile, because Apple strips the profile and re-signs.
#    The team is whatever the signature names; nothing here has to know it.
auth=$(codesign -dv --verbose=2 "$app" 2>&1)
team=$(printf '%s' "$auth" | sed -n 's/^Authority=[^(]*(\([A-Z0-9]\{10\}\)).*/\1/p' | head -1)
case "$auth" in
    *"TestFlight Beta Distribution"*)
        kind=testflight
        ok "signed" "TestFlight Beta Distribution — Apple re-signed this" ;;
    *"Apple Distribution: "*)
        kind=store
        ok "signed" "Apple Distribution, team ${team} — a Store build" ;;
    *"Developer ID Application: "*)
        kind=devid
        ok "signed" "Developer ID, team ${team} — the outside-the-Store hedge" ;;
    *"Apple Development"*)
        kind=dev
        ok "signed" "Apple Development — a local test build, not shippable" ;;
    *)
        kind=unknown
        bad "signed" "$(printf '%s' "$auth" | sed -n 's/^Authority=//p' | head -1)" ;;
esac
case "$auth" in
    *"Apple Root CA"*) ok "the chain reaches the Apple Root CA" "three authorities" ;;
    *) bad "the chain reaches the Apple Root CA" "it does not" ;;
esac

if codesign --verify --deep --strict "$app" 2>/dev/null; then
    ok "the signature verifies" "--deep --strict"
else
    bad "the signature verifies" "$(codesign --verify --deep --strict "$app" 2>&1 | head -1)"
fi

# 4. The entitlements, read back out of the signature rather than off the
#    file that was fed to it. `plutil -p` does not spell a boolean the same
#    way on every macOS: 15.7 prints `=> 1` and 26 prints `=> true`. Match
#    the key and accept either.
ents=$(codesign -d --entitlements - --xml "$app" 2>/dev/null | plutil -p - 2>/dev/null)
case "$ents" in
    *'"com.apple.security.app-sandbox" => 1'*|*'"com.apple.security.app-sandbox" => true'*)
        ok "the sandbox is in the signature" "app-sandbox" ;;
    *'com.apple.security.app-sandbox'*)
        bad "the sandbox is in the signature" "present but not true" ;;
    *) bad "the sandbox is in the signature" "absent — the build is not sandboxed" ;;
esac
# Restricted, and so the whole reason a Store build needs a profile and
# cannot run without one. A Developer ID or development build must *not*
# carry it: it would be refused at launch for exactly the reason the Store
# build is.
case "$kind" in
    store|testflight) wants_app_id=yes ;;
    *) wants_app_id=no ;;
esac
case "$wants_app_id,$ents" in
    yes,*"${team}.${bundle_id}"*)
        ok "the application identifier is there" "${team}.${bundle_id}" ;;
    yes,*)
        bad "the application identifier is there" "absent — the upload is refused" ;;
    no,*".${bundle_id}"*)
        bad "no application identifier" "present on a ${kind} build — it will not launch" ;;
    *)  ok "no application identifier" "correct for a ${kind} build" ;;
esac
# Declined deliberately: the profile grants it and this application touches
# no keychain, and a capability asked for and unused is a question at review.
case "$ents" in
    *keychain-access-groups*) bad "keychain-access-groups is declined" "it is present" ;;
    *) ok "keychain-access-groups is declined" "absent, as intended" ;;
esac

# 5. The profile has to be inside the bundle, and inside it *before* it was
#    signed; added afterwards, macOS calls the bundle damaged, which check 3
#    already catches, so this one is about presence. A TestFlight build
#    carries none and must not.
profile="${app}/Contents/embedded.provisionprofile"
if [ "$kind" != store ] && [ ! -f "$profile" ]; then
    ok "no provisioning profile" "correct for a ${kind} build"
elif [ -f "$profile" ]; then
    plist=$(mktemp)
    if security cms -D -i "$profile" -o "$plist" 2>/dev/null; then
        pname=$(plutil -extract Name raw -o - "$plist" 2>/dev/null)
        pexp=$(plutil -extract ExpirationDate raw -o - "$plist" 2>/dev/null)
        ok "a provisioning profile is embedded" "${pname:-unnamed}, expires ${pexp:-unknown}"
    else
        bad "a provisioning profile is embedded" "present but would not decode"
    fi
    rm -f "$plist"
else
    bad "a provisioning profile is embedded" "no embedded.provisionprofile"
fi

# 6. What the bundle claims about itself, which App Store Connect
#    deduplicates uploads by and a person reads in the About box.
short=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" \
    "${app}/Contents/Info.plist" 2>/dev/null)
build=$(/usr/libexec/PlistBuddy -c "Print :CFBundleVersion" \
    "${app}/Contents/Info.plist" 2>/dev/null)
note "the version it declares" "${short:-?} (build ${build:-?})"

# 7. Gatekeeper's verdict, which is not the signature's, and which for a
#    Store build is *rejection*, correctly: `spctl -a` assesses the Developer
#    ID and notarization policy, and a Store build is not distributed under
#    it. What is worth reporting is a verdict that does not match the
#    certificate the bundle carries.
gk=$(spctl -a -vvv "$app" 2>&1)
case "$kind,$gk" in
    *,*accepted*)
        note "Gatekeeper" "accepted — $(printf '%s' "$gk" | sed -n 's/.*source=//p' | head -1)" ;;
    store,*rejected*)
        note "Gatekeeper" "rejected, as a Store build correctly is" ;;
    devid,*"Unnotarized Developer ID"*)
        note "Gatekeeper" "rejected — unnotarized, which is the hedge's own step" ;;
    dev,*rejected*)
        note "Gatekeeper" "rejected, as a development build correctly is" ;;
    testflight,*rejected*)
        bad "Gatekeeper" "rejected a TestFlight build, which it should accept" ;;
    *)  bad "Gatekeeper" "$(printf '%s' "$gk" | tr '\n' ' ')" ;;
esac

# 7b. Whether Gatekeeper gets to decide at all, which is the variable that
#     actually governs a first launch. An unquarantined copy, anything built
#     here or carried over by scp, is not assessed, so a bundle that would be
#     refused after a download starts without a murmur.
if xattr -p com.apple.quarantine "$app" >/dev/null 2>&1; then
    note "it carries com.apple.quarantine" "$(xattr -p com.apple.quarantine "$app" 2>/dev/null)"
else
    note "it carries com.apple.quarantine" "no — so Gatekeeper is not consulted"
fi

# 8. Whether the App Sandbox actually engaged, which is a fact about a *run*
#    rather than about the bundle. The container directory is made on first
#    launch and by nothing else, so its absence after a launch means the
#    entitlement was carried and not honoured, the failure that looks like
#    success in every static check above.
container="${HOME}/Library/Containers/${bundle_id}"
if [ -d "$container" ]; then
    ok "a sandbox container exists" "$(basename "$container")"
else
    note "a sandbox container exists" "not yet — launch it once and re-run"
fi

# 9. Launch Services, which is what makes a double-click reach this
#    application at all. `-dump` is large; ask it only about our types.
lsregister=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
dump=$("$lsregister" -dump 2>/dev/null)
case "$dump" in
    *"$bundle_id"*) ok "Launch Services knows the bundle" "$bundle_id" ;;
    *) bad "Launch Services knows the bundle" "not registered — a double-click will not arrive" ;;
esac
for type in com.excelano.doclang-archive com.excelano.doclang-document; do
    case "$dump" in
        *"$type"*) ok "Launch Services knows the type" "$type" ;;
        *) bad "Launch Services knows the type" "$type is not declared by anything" ;;
    esac
done

echo
if [ "$findings" -eq 0 ]; then
    echo "Nothing mechanical is wrong with this install."
else
    echo "${findings} thing(s) to write down — in the commit, and in"
    echo "CHECKLIST.md only if the next person would run the list differently."
fi
echo "The rest needs eyes: the layout at 2x, the three icons, the frame, and what"
echo "Gatekeeper shows a person rather than what spctl reports."
exit 0
