#!/bin/python
import argparse
import matplotlib.pyplot as plt
import numpy as np
import scipy

parser = argparse.ArgumentParser()
parser.add_argument('filename', nargs="+")
args = parser.parse_args()

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
    filtered_data = np.convolve(v, filter_vals, 'valid')

    running_mean = 100.0
    alpha = 0.99
#    for i in range(len(filtered_data)):
#        running_mean = filtered_data[i] * (1-alpha) + alpha * running_mean
#        filtered_data -= running_mean
#
    plt.plot(time[:len(filtered_data)], filtered_data)
    plt.show()

    T = 1.0/40.0/60
    N = len(filtered_data)
    spectrum = scipy.fft.fft(filtered_data);
    xf = scipy.fft.fftfreq(N, T)[:N//2]

    plt.plot(xf, 2.0/N * np.abs(spectrum[0:N//2]))
    plt.show()

