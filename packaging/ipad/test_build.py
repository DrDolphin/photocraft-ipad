import importlib.util
from pathlib import Path
import plistlib
import stat
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location('ipad_build', Path(__file__).with_name('build.py'))
build = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build)


class IpadPackagingTests(unittest.TestCase):
    def test_ipad_only_files_integration_and_orientations(self):
        info = plistlib.loads(plistlib.dumps(build.app_info('com.personal.photocraft.ipad')))
        self.assertEqual(info['UIDeviceFamily'], [2])
        self.assertTrue(info['UIFileSharingEnabled'])
        self.assertTrue(info['LSSupportsOpeningDocumentsInPlace'])
        self.assertEqual(len(info['UISupportedInterfaceOrientations~ipad']), 4)
        self.assertEqual(info['CFBundleSupportedPlatforms'], ['iPhoneOS'])

    def test_simulator_is_distinct_from_device(self):
        self.assertEqual(build.app_info('com.personal.photocraft.ipad', True)['CFBundleSupportedPlatforms'], ['iPhoneSimulator'])

    def test_identifiers_cannot_contain_paths_or_shell_input(self):
        for invalid in ('', '../app', 'com.photo craft.ipad', 'com.photocraft;id', 'com.photocraft'):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                build.app_info(invalid)

    def test_ipa_layout_and_executable_permission(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            app = root / 'PhotoCraft.app'
            app.mkdir()
            binary = app / 'PhotoCraft'
            binary.write_bytes(b'test executable bytes')
            binary.chmod(0o755)
            (app / 'Info.plist').write_bytes(plistlib.dumps(build.app_info('com.personal.photocraft.ipad')))
            archive = root / 'app.ipa'
            build.archive_app(app, archive, ipa=True)
            with zipfile.ZipFile(archive) as zipped:
                self.assertEqual(sorted(zipped.namelist()), ['Payload/PhotoCraft.app/Info.plist', 'Payload/PhotoCraft.app/PhotoCraft'])
                if build.sys.platform != 'win32':
                    self.assertTrue((zipped.getinfo('Payload/PhotoCraft.app/PhotoCraft').external_attr >> 16) & stat.S_IXUSR)


if __name__ == '__main__':
    unittest.main()
