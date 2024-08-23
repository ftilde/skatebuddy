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

filter_vals0 = [
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


#FIR filter designed with
#http://t-filter.appspot.com
#
#sampling frequency: 25 Hz
#
#fixed point precision: 16 bits
#
#* 0.1 Hz - 0.5 Hz
#  gain = 0
#  desired attenuation = -41 dB
#  actual attenuation = n/a
#
#* 1 Hz - 3 Hz
#  gain = 1
#  desired ripple = 5 dB
#  actual ripple = n/a
#
#* 3.5 Hz - 12.5 Hz
#  gain = 0
#  desired attenuation = -40 dB
#  actual attenuation = n/a
#
#*/

filter_vals = [
  -238,
  -186,
  -165,
  -85,
  11,
  69,
  59,
  8,
  0,
  126,
  421,
  814,
  1131,
  1175,
  833,
  170,
  -576,
  -1086,
  -1134,
  -726,
  -141,
  190,
  -94,
  -1052,
  -2343,
  -3331,
  -3360,
  -2088,
  295,
  3081,
  5307,
  6156,
  5307,
  3081,
  295,
  -2088,
  -3360,
  -3331,
  -2343,
  -1052,
  -94,
  190,
  -141,
  -726,
  -1134,
  -1086,
  -576,
  170,
  833,
  1175,
  1131,
  814,
  421,
  126,
  0,
  8,
  59,
  69,
  11,
  -85,
  -165,
  -186,
  -238
]




print(sum(filter_vals))


def scale(i):
    factor = np.max(np.abs(i))
    return i/factor

for file in args.filename:
    v = np.loadtxt(file, delimiter=";", dtype=np.float32)
    time = np.array(range(0, len(v))) * 40
    #plt.plot(time, v)

    N = len(v)

    sample_rate = len(v)/(time[-1]/1000)
    #sos = scipy.signal.butter(5, 0.5, 'highpass', fs=sample_rate, output='sos')
    #filtered_data = scipy.signal.sosfiltfilt(sos, v)
    filtered_data0 = scale(np.convolve(v, filter_vals0, 'valid'))
    filtered_data = scale(np.convolve(v, filter_vals, 'valid'))

    running_mean = 100.0
    alpha = 0.99
#    for i in range(len(filtered_data)):
#        running_mean = filtered_data[i] * (1-alpha) + alpha * running_mean
#        filtered_data -= running_mean
#
    print(len(filtered_data)*40)
    plt.plot(time[:len(filtered_data0)], filtered_data0)
    #plt.plot(time[:len(filtered_data)], filtered_data)
    plt.show()

    T = 1.0/25.0/60

    l = 6000 // 40

    v = filtered_data0

    for i in range(N//l):
        sub_v = v[150*i:150*(i+1)]
        N = len(sub_v)
        spectrum = scipy.fft.rfft(sub_v);
        xf = scipy.fft.fftfreq(N, T)#[:N//2]

        xf = xf[3:23]
        spectrum = spectrum[3:23]
        plt.plot(xf, np.abs(spectrum))
    plt.show()

