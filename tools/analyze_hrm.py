#!/bin/python
import argparse
import matplotlib.pyplot as plt
import numpy as np
import scipy

parser = argparse.ArgumentParser()
parser.add_argument('filename', nargs="+")
args = parser.parse_args()

for file in args.filename:
    v = np.loadtxt(file, delimiter=";", dtype=np.float32)
    time = range(0, len(v))
    plt.plot(time, v)

    sample_rate = len(v)/(time[-1]/1000)
    sos = scipy.signal.butter(5, 0.5, 'highpass', fs=sample_rate, output='sos')
    filtered_data = scipy.signal.sosfiltfilt(sos, v)
    plt.plot(time, filtered_data)

plt.show()
