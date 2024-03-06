#!/bin/python
import argparse
import matplotlib.pyplot as plt
import numpy as np

parser = argparse.ArgumentParser()
parser.add_argument('filename', nargs="+")
args = parser.parse_args()

v_100 = 4.2;
v_80 = 3.95;
v_10 = 3.70;
v_0 = 3.3;

def piecewise_linear(voltage):

    if voltage > v_80:
        percentage = (voltage - v_80) * 20.0 / (v_100 - v_80) + 80.0
    elif voltage > v_10:
        percentage = (voltage - v_10) * 70.0 / (v_80 - v_10) + 10.0
    else:
        percentage = (voltage - v_0) * 10.0 / (v_10 - v_0)

    return percentage/100

ax = plt.gca()
ax.set_xlabel("voltage")
ax.set_ylabel("percentage")


lin_x = np.linspace(v_0, v_100, 100)
lin_y = [piecewise_linear(x) for x in lin_x]
ax.plot(lin_x, lin_y)

for file in args.filename:
    print(file)
    d = np.loadtxt(file, delimiter=";", dtype=np.float64)
    time = d[:, 0]
    voltage = d[:, 1]
    percentage = 1.0-time/np.max(time)
    ax.plot(voltage, percentage)

plt.show()

