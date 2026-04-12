import os
from PIL import Image
from src.utils import get_file_creation_date, check_for_file_creation_date_mismatch


def compress_image(args, src_file, new_file_path):
    if not new_file_path.lower().endswith(".webp"):
        new_file_path += ".webp"
    temp_file_path = new_file_path + ".tmp.webp"

    if os.path.exists(new_file_path):
        print(f'skipping "{new_file_path}"')
        return

    if os.path.exists(temp_file_path):
        print(f'deleting partial output from previous run: "{temp_file_path}"')
        os.remove(temp_file_path)
    print(f'converting "{src_file}" ...')
    im = Image.open(src_file).convert("RGB")

    exif_date, metadata_date = get_file_creation_date(src_file, True)
    if not args.ignore_timestamps and check_for_file_creation_date_mismatch(src_file, exif_date, metadata_date):
        return

    # resize large images
    if args.image_max_pixels > 0:
        im.thumbnail((args.image_max_pixels, args.image_max_pixels), Image.LANCZOS)
    im.save(temp_file_path,
            exif=im.info['exif'],   # Preserve exif-info
            method=6,               # Quality/speed trade-off (0=fast, 6=slower-better). Defaults to 4
            quality=args.webpq,     # For lossy, 0 gives the smallest size and 100 the largest
            lossless=False)         # Enable lossy compression

    os.utime(temp_file_path, (metadata_date.timestamp(), metadata_date.timestamp()))
    os.rename(temp_file_path, new_file_path)
