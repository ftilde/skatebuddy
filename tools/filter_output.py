#!/usr/bin/python

import fileinput
import os.path
import base64
import traceback

file_begin_prefix = "===== "
file_end_prefix = "====="
out_base_dir = "downloaded"

current_out_file = None

for line in fileinput.input():
    try:
        if current_out_file is None:
            if line.startswith(file_begin_prefix):
                file_path = line[len(file_begin_prefix):].strip()
                out_file_path = os.path.join(out_base_dir, file_path)
                out_file_dir = os.path.dirname(out_file_path)
                os.makedirs(out_file_dir, exist_ok=True)
                current_out_file = open(out_file_path, "wb")
                print(f"Downloading file {file_path}")
            else:
                print(line, end="")
        else:
            if line.startswith(file_end_prefix):
                current_out_file.close()
                current_out_file = None
            else:
                current_out_file.write(base64.b64decode(line.strip()))
    except:
        traceback.print_exc()
        if current_out_file is not None:
            current_out_file.close()
            current_out_file = None
