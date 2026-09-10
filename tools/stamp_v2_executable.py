"""Apply the existing product icon and version resources to our Electron copy."""
from pathlib import Path
import sys

from PyInstaller.utils.win32.icon import CopyIcons_FromIco, IconFile
from PyInstaller.utils.win32.winresource import get_resources, add_or_update_resource
from PyInstaller.utils.win32.versioninfo import (
    FixedFileInfo, StringFileInfo, StringStruct, StringTable,
    VarFileInfo, VarStruct, VSVersionInfo, write_version_info_to_executable,
    read_version_info_from_executable,
)


def main():
    executable, version = Path(sys.argv[1]).resolve(strict=True), sys.argv[2]
    numbers = tuple(int(value) for value in version.split('-')[0].split('.')) + (0,)
    if len(numbers) != 4 or any(not 0 <= value <= 65535 for value in numbers):
        raise ValueError('Expected a three-part application version')
    icon_path = Path(__file__).resolve().parents[1] / 'assets/nioh3-scroll-generator.ico'
    original = get_resources(str(executable), types=[3, 14, 16])
    languages = {language for names in original.values() for values in names.values() for language in values} | {0}
    CopyIcons_FromIco(str(executable), [str(icon_path)])
    strings = {
        'CompanyName': 'MasterBayesian and Saber_Li',
        'FileDescription': 'Nioh 3 Scroll Editor',
        'FileVersion': version,
        'InternalName': 'Nioh3ScrollEditorV2',
        'OriginalFilename': 'Nioh3ScrollEditorV2.exe',
        'ProductName': 'Nioh 3 Scroll Editor',
        'ProductVersion': version,
    }
    write_version_info_to_executable(str(executable), VSVersionInfo(
        ffi=FixedFileInfo(filevers=numbers, prodvers=numbers, mask=0x3F,
                          flags=0, OS=0x40004, fileType=1, subtype=0, date=(0, 0)),
        kids=[StringFileInfo([StringTable('040904B0', [StringStruct(key, value) for key, value in strings.items()])]),
              VarFileInfo([VarStruct('Translation', [1033, 1200])])],
    ))
    # Electron ships English resources. Replace them too, otherwise Windows can
    # select the old icon/version when it chooses language 1033 over neutral.
    neutral = get_resources(str(executable), types=[3, 14, 16], languages=[0])
    for kind, names in neutral.items():
        for name, values in names.items():
            add_or_update_resource(str(executable), values[0], kind, names=[name], languages=sorted(languages))
    actual = read_version_info_from_executable(str(executable)).ffi
    if (actual.fileVersionMS, actual.fileVersionLS, actual.productVersionMS, actual.productVersionLS) != (
            (numbers[0] << 16) + numbers[1], (numbers[2] << 16) + numbers[3],
            (numbers[0] << 16) + numbers[1], (numbers[2] << 16) + numbers[3]):
        raise ValueError('Executable version resource verification failed')
    images = get_resources(str(executable), types=[3])[3]
    for index, expected in enumerate(IconFile(str(icon_path)).images, 1):
        if any(raw != expected for raw in images[index].values()):
            raise ValueError('Executable icon resource verification failed')


if __name__ == '__main__':
    main()
