import os
from src.utils import get_file_creation_date


def fix_dates(src_file, is_image):
    exif_date, metadata_date = get_file_creation_date(src_file, is_image)

    if metadata_date is None:
        print(f'ERROR - File "{src_file}": unable to fetch metadata-date!')
        exit(1)

    if exif_date is None:
        print(f'File "{src_file}" is missing an exif_date. Setting metadata-date {metadata_date}')
        print("ERROR - this functionality is not yet implemented!")
        exit(1)

    if abs((exif_date.timestamp() - metadata_date.timestamp())) > 24 * 60 * 60:
        if (exif_date.timestamp() - metadata_date.timestamp()) < 0:
            print(f'File "{src_file}" date mismatch! Using exif-date as it is older. NEW DATE: {exif_date}')
            os.utime(src_file, (exif_date.timestamp(), exif_date.timestamp()))
        else:
            print(f'File "{src_file}" date mismatch! Using metadata-date as it is older. NEW DATE: {metadata_date}')
            print("ERROR - this functionality is not yet implemented!")
            exit(1)
    else:
        print(f'File "{src_file}" dates ok.')
