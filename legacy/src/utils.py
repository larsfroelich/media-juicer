from datetime import datetime, timezone

from PIL.ExifTags import TAGS
from hachoir.parser import createParser
from hachoir.metadata import extractMetadata
from PIL import Image
import os


def ends_with_any(test_input, endings):
    for ending in endings:
        if test_input.endswith(ending):
            return True
    return False


def check_for_file_creation_date_mismatch(path, exif_date, metadata_date):
    if exif_date is None or metadata_date is None or abs((exif_date.timestamp() - metadata_date.timestamp())) > 24 * 60 * 60:
        print(f"File creation date mismatch! ({path}):  Exif Date: '{exif_date}' Metadata Date: '{metadata_date}'")
        return True
    elif exif_date is None or metadata_date is None or abs((exif_date.timestamp() - metadata_date.timestamp())) > 15 * 60:
        print(f"WARNING: Slight file creation date mismatch of {abs((exif_date.timestamp() - metadata_date.timestamp()))}"
              f" seconds! ({path}):  Exif Date: '{exif_date}' Metadata Date: '{metadata_date}'")
        return False
    else:
        return False


def get_file_creation_date(path, is_image):
    exif_date = get_file_exif_date(path, is_image)
    if exif_date is None:
        print("Unable to determine exif date for file %s" % path)

    metadata_date = get_file_metadata_date(path)
    return exif_date, metadata_date


def get_file_metadata_date(path):
    metadata_timestamp = datetime.fromtimestamp(os.path.getmtime(path), tz=timezone.utc)
    if metadata_timestamp.year < 1980:
        print(f"Metadata timestamp too old! {metadata_timestamp}")
        return None
    return metadata_timestamp


def get_file_exif_date(path, is_image):
    exif_date_created = None
    # get create date from within the file
    if is_image:
        im = Image.open(path)
        exif = im.getexif()
        exif_date_created = (get_exif_field(exif, "DateTimeDigitized")
                             or get_exif_field(exif, "DateTimeOriginal")
                             or get_exif_field(exif, "DateTime")
                             or get_exif_field(exif, "TimeStamp"))
        exif_timezone_offset = (get_exif_field(exif, "OffsetTimeDigitized")
                                or get_exif_field(exif, "OffsetTimeOriginal")
                                or get_exif_field(exif, "OffsetTime"))
        if not exif_timezone_offset and exif_date_created[-6] in ['-', '+']:
            exif_timezone_offset = exif_date_created[-6:]
        if not exif_date_created or len(exif_date_created) < 1:
            print("Unable to determine image creation date!")
            return None
        if not exif_timezone_offset:
            print("Unable to determine image creation date timezone offset. Assuming UTC")

        if exif_date_created[-6] in ['-', '+']:
            exif_date_created = exif_date_created[:-6]
        if exif_timezone_offset:
            exif_date_created = datetime.strptime(exif_date_created + " " + exif_timezone_offset, '%Y:%m:%d %H:%M:%S %z')
        else:
            exif_date_created = datetime.strptime(exif_date_created + " +00:00", '%Y:%m:%d %H:%M:%S %z')
    else:
        parser = createParser(path)
        if not parser:
            print("Unable to parse file %s" % path)
            return None
        with parser:
            try:
                metadata = extractMetadata(parser)
            except Exception as err:
                print("Metadata extraction error: %s" % err)
                metadata = None
        if not metadata:
            print("Unable to extract metadata")
            return None
        for line in metadata.exportPlaintext():
            if line.split(':')[0] == '- Creation date':
                exif_date_created = datetime.strptime(line, "- Creation date: %Y-%m-%d %H:%M:%S")
    if exif_date_created is not None and exif_date_created.year < 1980:
        print(f"Exif timestamp too old! {exif_date_created}")
        return None
    return exif_date_created


def get_exif_field(exif, field):
    if field not in TAGS.values():
        raise ValueError(f"Unknown EXIF field: {field}")

    for tag_id, value in exif.items():
        if TAGS.get(tag_id) == field:
            return value

    return None



