#!/usr/bin/env python3
"""Build a real Rust/Metal iPad app with Apple's SDK, and optionally smoke-test it.

The device IPA intentionally contains no personal signing identity. Sign it for
your own iPad after the build; the script never reads Apple account credentials.
"""
import argparse
import datetime as dt
import json
import os
from pathlib import Path
import platform
import plistlib
import re
import shutil
import subprocess
import sys
import time
import zipfile

ROOT = Path(__file__).resolve().parents[2]
MINIMUM_OS = '17.0'
DEFAULT_ID = 'com.personal.photocraft.ipad'


def run(args, *, env=None, capture=False):
    result = subprocess.run([str(arg) for arg in args], env=env, check=True,
                            text=True, stdout=subprocess.PIPE if capture else None)
    return result.stdout.strip() if capture else None


def app_info(bundle_id, simulator=False):
    if not re.fullmatch(r'[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+){2,}', bundle_id):
        raise ValueError('Use a reverse-DNS bundle identifier without spaces')
    return {
        'CFBundleDevelopmentRegion': 'en',
        'CFBundleDisplayName': 'PhotoCraft',
        'CFBundleExecutable': 'PhotoCraft',
        'CFBundleIdentifier': bundle_id,
        'CFBundleInfoDictionaryVersion': '6.0',
        'CFBundleName': 'PhotoCraft',
        'CFBundlePackageType': 'APPL',
        'CFBundleShortVersionString': '0.16.0',
        'CFBundleVersion': '1',
        'CFBundleSupportedPlatforms': ['iPhoneSimulator' if simulator else 'iPhoneOS'],
        'MinimumOSVersion': MINIMUM_OS,
        'LSRequiresIPhoneOS': True,
        'UIDeviceFamily': [2],
        'UIFileSharingEnabled': True,
        'LSSupportsOpeningDocumentsInPlace': True,
        'UIApplicationSupportsIndirectInputEvents': True,
        'UIStatusBarHidden': True,
        'UIViewControllerBasedStatusBarAppearance': False,
        'UIRequiresFullScreen': False,
        'UISupportedInterfaceOrientations~ipad': [
            'UIInterfaceOrientationPortrait', 'UIInterfaceOrientationPortraitUpsideDown',
            'UIInterfaceOrientationLandscapeLeft', 'UIInterfaceOrientationLandscapeRight',
        ],
        'UILaunchScreen': {},
        'NSHighResolutionCapable': True,
    }


def archive_app(bundle, output, ipa):
    """Preserve executable permissions and the IPA's required Payload directory."""
    bundle, output = Path(bundle), Path(output)
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
        for path in sorted(bundle.rglob('*')):
            if not path.is_file():
                continue
            relative = Path('Payload') / bundle.name / path.relative_to(bundle) if ipa else Path(bundle.name) / path.relative_to(bundle)
            archive.write(path, relative.as_posix())


def icons(bundle, work, simulator):
    assets = work / 'Assets.xcassets'
    iconset = assets / 'AppIcon.appiconset'
    iconset.mkdir(parents=True, exist_ok=True)
    (assets / 'Contents.json').write_text(json.dumps({'info': {'version': 1, 'author': 'xcode'}}))
    source = ROOT / 'assets/app-icon/hicolor/512x512/apps/ai.storyteller.photocraft.png'
    images = []
    for size, scale, pixels in [('20', '1x', 20), ('20', '2x', 40), ('29', '1x', 29),
                               ('29', '2x', 58), ('40', '1x', 40), ('40', '2x', 80),
                               ('76', '1x', 76), ('76', '2x', 152), ('83.5', '2x', 167)]:
        filename = f'ipad-{pixels}.png'
        run(['/usr/bin/sips', '--resampleHeightWidth', pixels, pixels, source,
             '--out', iconset / filename], capture=True)
        images.append({'idiom': 'ipad', 'size': f'{size}x{size}', 'scale': scale, 'filename': filename})
    (iconset / 'Contents.json').write_text(json.dumps({'images': images, 'info': {'version': 1, 'author': 'xcode'}}))
    partial = work / 'asset-info.plist'
    run(['xcrun', 'actool', assets, '--compile', bundle, '--platform',
         'iphonesimulator' if simulator else 'iphoneos', '--minimum-deployment-target', MINIMUM_OS,
         '--target-device', 'ipad', '--app-icon', 'AppIcon', '--output-partial-info-plist', partial,
         '--product-type', 'com.apple.product-type.application'])
    return plistlib.loads(partial.read_bytes())


def smoke_test(bundle, bundle_id, output):
    inventory = json.loads(run(['xcrun', 'simctl', 'list', 'devices', 'available', '--json'], capture=True))
    candidates = [device for runtime, devices in inventory['devices'].items() if '.iOS-' in runtime
                  for device in devices if device.get('isAvailable') and device['name'].startswith('iPad')]
    if not candidates:
        raise RuntimeError('No installed iPad Simulator is available; install an iOS runtime in Xcode')
    device = candidates[-1]
    udid = device['udid']
    if device.get('state') != 'Booted':
        run(['xcrun', 'simctl', 'boot', udid])
    run(['xcrun', 'simctl', 'bootstatus', udid, '-b'])
    run(['xcrun', 'simctl', 'install', udid, bundle])
    launched = run(['xcrun', 'simctl', 'launch', udid, bundle_id], capture=True)
    match = re.search(r':\s*(\d+)\s*$', launched)
    if not match:
        raise RuntimeError(f'Simulator did not report an application process: {launched}')
    pid = int(match.group(1))
    time.sleep(12)
    os.kill(pid, 0)
    screenshot = output / 'ipad-simulator.png'
    run(['xcrun', 'simctl', 'io', udid, 'screenshot', screenshot])
    run(['xcrun', 'simctl', 'terminate', udid, bundle_id])
    return {'device': device['name'], 'native_process_survived_12_seconds': True,
            'screenshot': screenshot.name}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--simulator', action='store_true')
    parser.add_argument('--smoke-test', action='store_true')
    parser.add_argument('--check-prerequisites', action='store_true')
    parser.add_argument('--bundle-id', default=DEFAULT_ID)
    parser.add_argument('--output', type=Path, default=ROOT / 'dist/ipad')
    args = parser.parse_args()
    if sys.platform != 'darwin':
        print('Native iPad linking and packaging require Xcode on macOS. Windows can run cargo check '
              '--target aarch64-apple-ios. Use the manual iPad Native GitHub Actions workflow for a cloud Mac build.', file=sys.stderr)
        return 2
    if args.smoke_test and not args.simulator:
        parser.error('--smoke-test requires --simulator')
    sdk = 'iphonesimulator' if args.simulator else 'iphoneos'
    sdkroot = run(['xcrun', '--sdk', sdk, '--show-sdk-path'], capture=True)
    run(['xcrun', '--find', 'actool'], capture=True)
    if args.check_prerequisites:
        print(json.dumps({'sdk': sdk, 'sdkroot': sdkroot, 'minimum_ipados': MINIMUM_OS}, indent=2))
        return 0
    target = ('aarch64-apple-ios-sim' if platform.machine() in ('arm64', 'aarch64') else 'x86_64-apple-ios') if args.simulator else 'aarch64-apple-ios'
    run(['rustup', 'target', 'add', target])
    info = app_info(args.bundle_id, args.simulator)
    environment = dict(os.environ, SDKROOT=sdkroot, IPHONEOS_DEPLOYMENT_TARGET=MINIMUM_OS)
    environment.setdefault('CARGO_BUILD_JOBS', '2')
    run(['cargo', 'build', '--locked', '--package', 'photocraft-ipad', '--profile', 'ipad-release',
         '--target', target, '--manifest-path', ROOT / 'Cargo.toml'], env=environment)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    bundle = output / 'PhotoCraft.app'
    if bundle.exists():
        raise RuntimeError(f'Use a fresh output directory; {bundle} already exists')
    bundle.mkdir()
    target_dir = Path(environment.get('CARGO_TARGET_DIR', ROOT / 'target'))
    if not target_dir.is_absolute():
        target_dir = ROOT / target_dir
    executable = target_dir / target / 'ipad-release/photocraft-ipad'
    shutil.copy2(executable, bundle / 'PhotoCraft')
    (bundle / 'PhotoCraft').chmod(0o755)
    binary_type = run(['file', bundle / 'PhotoCraft'], capture=True)
    build_version = run(['xcrun', 'vtool', '-show-build', bundle / 'PhotoCraft'], capture=True)
    expected_platform = 'IOSSIMULATOR' if args.simulator else 'IOS'
    if 'Mach-O' not in binary_type or not re.search(r'platform\s+' + expected_platform + r'\b', build_version):
        raise RuntimeError('The executable is not built for the requested Apple platform')
    info.update(icons(bundle, output / 'asset-build', args.simulator))
    (bundle / 'Info.plist').write_bytes(plistlib.dumps(info))
    licenses = bundle / 'Licenses'
    licenses.mkdir()
    for name in ('LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE', 'ATTRIBUTION.md'):
        shutil.copy2(ROOT / name, licenses / name)
    shutil.copy2(ROOT / 'assets/app-icon/LICENSE.txt', licenses / 'AppIcon-LICENSE.txt')
    report = {'version': '0.16.0-ipad.1', 'target': target, 'bundle_identifier': args.bundle_id,
              'minimum_ipados': MINIMUM_OS, 'native_renderer': 'wgpu / Metal', 'binary': binary_type,
              'apple_build_version': build_version, 'checked_at': dt.datetime.now(dt.timezone.utc).isoformat(),
              'signing': 'simulator ad-hoc' if args.simulator else 'unsigned; personal signing is required before installation'}
    if args.simulator:
        run(['codesign', '--force', '--sign', '-', '--timestamp=none', bundle])
        run(['codesign', '--verify', '--deep', '--strict', bundle])
        archive = output / 'PhotoCraft-iPad-Simulator.zip'
    else:
        archive = output / 'PhotoCraft-iPad-unsigned.ipa'
    archive_app(bundle, archive, ipa=not args.simulator)
    if args.smoke_test:
        report['simulator_smoke_test'] = smoke_test(bundle, args.bundle_id, output)
    (output / 'build-report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'archive': str(archive), 'report': report}, indent=2))
    return 0


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f'iPad build failed: {error}', file=sys.stderr)
        raise SystemExit(1)
