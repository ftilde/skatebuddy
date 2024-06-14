#!/bin/python
import argparse
import matplotlib.pyplot as plt
import numpy as np
import scipy

parser = argparse.ArgumentParser()
parser.add_argument('filename', nargs="+")
args = parser.parse_args()

filter_vals = np.array([
  6,
  7,
  8,
  5,
  -2,
  -9,
  -14,
  -14,
  -10,
  -4,
  0,
  1,
  0,
  -1,
  0,
  3,
  5,
  6,
  5,
  4,
  5,
  8,
  10,
  9,
  5,
  0,
  -1,
  2,
  6,
  6,
  0,
  -11,
  -20,
  -21,
  -13,
  -3,
  -3,
  -18,
  -42,
  -59,
  -51,
  -12,
  46,
  98,
  119,
  98,
  46,
  -12,
  -51,
  -59,
  -42,
  -18,
  -3,
  -3,
  -13,
  -21,
  -20,
  -11,
  0,
  6,
  6,
  2,
  -1,
  0,
  5,
  9,
  10,
  8,
  5,
  4,
  5,
  6,
  5,
  3,
  0,
  -1,
  0,
  1,
  0,
  -4,
  -10,
  -14,
  -14,
  -9,
  -2,
  5,
  8,
  7,
  6
])

#/*
#
#FIR filter designed with
#http://t-filter.appspot.com
#
#sampling frequency: 25 Hz
#
#fixed point precision: 10 bits
#
#* 0.1 Hz - 0.6 Hz
#  gain = 0
#  desired attenuation = -32.07 dB
#  actual attenuation = n/a
#
#* 0.9 Hz - 3.5 Hz
#  gain = 1
#  desired ripple = 5 dB
#  actual ripple = n/a
#
#* 4 Hz - 12.5 Hz
#  gain = 0
#  desired attenuation = -20 dB
#  actual attenuation = n/a
#
#*/

#define FILTER_TAP_NUM 67

filter_vals = [
  -30,
  -12,
  -5,
  5,
  12,
  13,
  9,
  3,
  0,
  4,
  10,
  14,
  12,
  5,
  -3,
  -5,
  0,
  7,
  9,
  1,
  -12,
  -22,
  -23,
  -13,
  -2,
  -1,
  -17,
  -42,
  -60,
  -51,
  -12,
  47,
  99,
  120,
  99,
  47,
  -12,
  -51,
  -60,
  -42,
  -17,
  -1,
  -2,
  -13,
  -23,
  -22,
  -12,
  1,
  9,
  7,
  0,
  -5,
  -3,
  5,
  12,
  14,
  10,
  4,
  0,
  3,
  9,
  13,
  12,
  5,
  -5,
  -12,
  -30
]



print(sum(filter_vals))


for file in args.filename:
    v = np.loadtxt(file, delimiter=";", dtype=np.float32)
    time = range(0, len(v))
    plt.plot(time, v)

    sample_rate = len(v)/(time[-1]/1000)
    #sos = scipy.signal.butter(5, 0.5, 'highpass', fs=sample_rate, output='sos')
    #filtered_data = scipy.signal.sosfiltfilt(sos, v)
    filtered_data = np.convolve(v, filter_vals, 'same')

    running_mean = 100.0
    alpha = 0.99
#    for i in range(len(filtered_data)):
#        running_mean = filtered_data[i] * (1-alpha) + alpha * running_mean
#        filtered_data -= running_mean
#
    plt.plot(time, filtered_data)

plt.show()
