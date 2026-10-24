"""Enable UTF-8 Win32 paths in the bundled third-party runtime, preserving its manifest."""
import ctypes
from ctypes import wintypes
import xml.etree.ElementTree as ET

ASSEMBLY = "urn:schemas-microsoft-com:asm.v1"
APPLICATION = "urn:schemas-microsoft-com:asm.v3"
SETTINGS = "http://schemas.microsoft.com/SMI/2019/WindowsSettings"


def utf8_manifest(original):
    root = ET.fromstring(original) if original else ET.Element(f"{{{ASSEMBLY}}}assembly", manifestVersion="1.0")
    application = root.find(f"{{{APPLICATION}}}application")
    if application is None:
        application = ET.SubElement(root, f"{{{APPLICATION}}}application")
    settings = application.find(f"{{{APPLICATION}}}windowsSettings")
    if settings is None:
        settings = ET.SubElement(application, f"{{{APPLICATION}}}windowsSettings")
    codepage = settings.find(f"{{{SETTINGS}}}activeCodePage")
    if codepage is None:
        codepage = ET.SubElement(settings, f"{{{SETTINGS}}}activeCodePage")
    codepage.text = "UTF-8"
    return ET.tostring(root, encoding="utf-8", xml_declaration=True)


def enable_utf8(executable):
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    pointer = ctypes.c_void_p
    kernel.LoadLibraryExW.argtypes = [wintypes.LPCWSTR, pointer, wintypes.DWORD]
    kernel.LoadLibraryExW.restype = pointer
    kernel.FreeLibrary.argtypes = [pointer]
    kernel.FindResourceW.argtypes = [pointer, pointer, pointer]
    kernel.FindResourceW.restype = pointer
    kernel.SizeofResource.argtypes = [pointer, pointer]
    kernel.SizeofResource.restype = wintypes.DWORD
    kernel.LoadResource.argtypes = [pointer, pointer]
    kernel.LoadResource.restype = pointer
    kernel.LockResource.argtypes = [pointer]
    kernel.LockResource.restype = pointer
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, pointer, pointer, pointer, wintypes.WORD, wintypes.LPARAM)
    kernel.EnumResourceLanguagesW.argtypes = [pointer, pointer, pointer, callback_type, wintypes.LPARAM]
    kernel.BeginUpdateResourceW.argtypes = [wintypes.LPCWSTR, wintypes.BOOL]
    kernel.BeginUpdateResourceW.restype = pointer
    kernel.UpdateResourceW.argtypes = [pointer, pointer, pointer, wintypes.WORD, pointer, wintypes.DWORD]
    kernel.UpdateResourceW.restype = wintypes.BOOL
    kernel.EndUpdateResourceW.argtypes = [pointer, wintypes.BOOL]
    kernel.EndUpdateResourceW.restype = wintypes.BOOL

    module = kernel.LoadLibraryExW(str(executable), None, 0x22)
    if not module:
        raise ctypes.WinError(ctypes.get_last_error())
    languages = []
    original = None
    try:
        @callback_type
        def language_found(module, kind, name, language, parameter):
            languages.append(language)
            return True
        kernel.EnumResourceLanguagesW(module, 24, 1, language_found, 0)
        resource = kernel.FindResourceW(module, 1, 24)
        if resource:
            size = kernel.SizeofResource(module, resource)
            data = kernel.LockResource(kernel.LoadResource(module, resource))
            if not data or not size:
                raise ctypes.WinError(ctypes.get_last_error())
            original = ctypes.string_at(data, size)
    finally:
        kernel.FreeLibrary(module)
    manifest = utf8_manifest(original)
    buffer = ctypes.create_string_buffer(manifest)
    update = kernel.BeginUpdateResourceW(str(executable), False)
    if not update:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        for language in languages or [0]:
            if not kernel.UpdateResourceW(update, 24, 1, language, buffer, len(manifest)):
                raise ctypes.WinError(ctypes.get_last_error())
    except BaseException:
        kernel.EndUpdateResourceW(update, True)
        raise
    if not kernel.EndUpdateResourceW(update, False):
        raise ctypes.WinError(ctypes.get_last_error())
