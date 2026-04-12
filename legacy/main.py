import os.path

from src.compress_image import compress_image
from src.fix_dates import fix_dates
from src.parse_args import parse_program_args
from src.compress_video import compress_video
from src.list_files import list_files
from src.list_folders import list_folders
from src.mk_folder_if_not_exist import mk_folder_if_not_exist
from src.utils import ends_with_any
from pillow_heif import register_heif_opener  # HEIF support

register_heif_opener()  # HEIF support

args = parse_program_args()
print(f'Config: {args}')
src_folder_path = args.folder_path
src_path_split = os.path.split(os.path.normpath(args.folder_path))
out_folder_path = os.path.join(src_path_split[0], src_path_split[1] + "_compressed")

if args.mode == "fixdates":
    print("File date-fixing mode!\n")
else:
    print(f'source folder path: "{src_folder_path}"')
    print(f'compressed folder path: "{out_folder_path}"\n')
    mk_folder_if_not_exist(out_folder_path)
    for folder in list_folders(src_folder_path):
        new_folder_path = os.path.join(out_folder_path, os.path.relpath(folder, src_folder_path))
        mk_folder_if_not_exist(new_folder_path)

files = list_files(src_folder_path)
if args.only is not None:
    files = list(filter(lambda x: x.endswith(args.only), files))


def is_file_path_video(file_path):
    return ends_with_any(file_path.lower(), [".mp4", ".mov", ".mkv", ".avi", ".mts", ".vob", ".ts", ".mpg", ".mpeg"])


def is_file_path_image(file_path):
    return ends_with_any(file_path.lower(), [".jpg", ".jpeg", ".png", ".bmp", ".exif"])


if args.mode == "fixdates" or args.mode == "all":
    files_to_process = list(filter(lambda x: is_file_path_image(x) or is_file_path_video(x), files))
elif args.mode == "videos":
    files_to_process = list(filter(lambda x: is_file_path_video(x), files))
elif args.mode == "images":
    files_to_process = list(filter(lambda x: is_file_path_image(x), files))
else:
    print("invalid mode")
    exit(0)

total_bytes_to_process = sum(map(lambda x: os.stat(x).st_size, files_to_process), 0)

nr_processed_files = 0
nr_bytes_processed = 0
for file in files:
    new_file_path = os.path.join(out_folder_path, os.path.relpath(file, src_folder_path))

    processed_additional_file = False
    if is_file_path_video(file):
        if args.mode == "fixdates":
            fix_dates(file, False)
            processed_additional_file = True
        elif args.mode == "all" or args.mode == "videos":
            compress_video(args, file, new_file_path)
            processed_additional_file = True

    if is_file_path_image(file):
        if args.mode == "fixdates":
            fix_dates(file, True)
            processed_additional_file = True
        elif args.mode == "all" or args.mode == "images":
            compress_image(args, file, new_file_path)
            processed_additional_file = True

    if processed_additional_file:
        nr_processed_files += 1
        nr_bytes_processed += os.stat(file).st_size
        print(f"Processed {nr_processed_files}/{files_to_process.__len__()} files ({round(nr_bytes_processed/1e6, 1)}MB/{round(total_bytes_to_process/1e6, 1)}MB - {round(nr_bytes_processed/total_bytes_to_process*100.0, 2)}%).")


print(f"Processed a total of {nr_processed_files} files.")
