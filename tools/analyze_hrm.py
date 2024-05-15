#!/bin/python
import argparse
import matplotlib.pyplot as plt
import numpy as np

parser = argparse.ArgumentParser()
parser.add_argument('filename', nargs="+")
args = parser.parse_args()

for file in args.filename:
    d = np.loadtxt(file, delimiter=";", dtype=np.float32)
    time = d[:, 0]
    v = d[:, 1]
    plt.plot(time, v)

plt.show()
