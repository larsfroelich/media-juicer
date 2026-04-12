import os


def mk_folder_if_not_exist(path):
    if not os.path.exists(path):
        os.mkdir(path)

