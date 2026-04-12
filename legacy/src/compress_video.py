import os
import subprocess
import shutil

from src.utils import get_file_creation_date, check_for_file_creation_date_mismatch


def compress_video(args, src_file, new_file_path):
    if not new_file_path.lower().endswith(".mp4"):
        new_file_path += ".mp4"
    temp_file_path = new_file_path + ".tmp.mp4"

    if os.path.exists(new_file_path):
        if not args.replace:
            print(f'skipping "{new_file_path}"')
            return
    else:
        if os.path.exists(temp_file_path):
            print(f'deleting partial output from previous run: "{temp_file_path}"')
            os.remove(temp_file_path)

        exif_date, metadata_date = get_file_creation_date(src_file, False)
        if not args.ignore_timestamps and check_for_file_creation_date_mismatch(src_file, exif_date, metadata_date):
            return

        print(f'converting "{src_file}" ...')
        ffmpeg_arguments = [
            "ffmpeg",
            "-i", src_file,
            "-map_metadata", "0",
            "-c:v", "libx265",
            "-x265-params", f'crf={args.crf}',
            "-preset", f'{args.ffmpeg_speed}',
            "-c:a", "aac",
            "-tune", "fastdecode"
        ]

        # resize large images
        if args.video_max_pixels > 0:
            ffmpeg_arguments.append("-filter:v")
            ffmpeg_arguments.append(f"scale='min({args.video_max_pixels},iw)':min'({args.video_max_pixels},ih)'"
                                    f":force_original_aspect_ratio=decrease")
        ffmpeg_arguments.append(temp_file_path)

        file_size_input = os.path.getsize(src_file)
        ffmpeg_result = subprocess.run(ffmpeg_arguments, capture_output=True)
        if ffmpeg_result.returncode != 0:
            print("Failed to compress video! output: ")
            print(ffmpeg_result.stdout)
            print(ffmpeg_result.stderr)
            return
        file_size_output = os.path.getsize(temp_file_path)

        if file_size_output > file_size_input:
            print(f"compression did not lead to smaller file "
                  f"({file_size_input/1000000.0}M -> {file_size_output/1000000.0}M). Copying input!")
            os.remove(temp_file_path)
            shutil.copy(src_file, temp_file_path)

        os.utime(temp_file_path, (metadata_date.timestamp(), metadata_date.timestamp()))
        os.rename(temp_file_path, new_file_path)

    if os.path.exists(new_file_path) and args.replace:
        os.remove(src_file)
        src_file_path_modified = src_file
        if not src_file_path_modified.lower().endswith(".mp4"):
            src_file_path_modified += ".mp4"

        shutil.copy(new_file_path, src_file_path_modified)
        print(f'Replaced input file "{src_file}" with processed file.')
