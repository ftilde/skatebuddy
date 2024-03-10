#!/bin/python
import argparse
import matplotlib.pyplot as plt
import numpy as np
import torch
from torch import nn
from collections import OrderedDict
from scipy.optimize import curve_fit
import scipy


parser = argparse.ArgumentParser()
parser.add_argument('filename', nargs="+")
args = parser.parse_args()

ax = plt.gca()
ax.set_xlabel("voltage")
ax.set_ylabel("percentage")

all_voltages = []
all_percentages = []

for file in args.filename:
    print(file)
    d = np.loadtxt(file, delimiter=";", dtype=np.float32)
    time = d[:, 0]
    voltage = d[:, 1]
    percentage = 1.0-time/np.max(time)
    #ax.plot(voltage, percentage)
    all_voltages.append(voltage)
    all_percentages.append(percentage)

av = np.concatenate(all_voltages)

v0 = np.min(av)
v3 = np.max(av)

def resample(voltages, percentages):
    perm = np.argsort(voltages)
    voltages = voltages[perm]
    percentages = percentages[perm]

    lut_perc = []
    i = 0
    for x in lin_x:
        j=i
        while j < len(voltages) and x > voltages[j]:
            j+=1
        lut_perc.append(np.mean(percentages[i:(j+1)]))
        i=min(j, len(voltages)-1)

    return np.array(lut_perc)

lin_x = np.linspace(v0, v3, 100, dtype=np.float32)

all_v = []
all_perc = []

for v, p in zip(all_voltages, all_percentages):
    lut_perc = resample(v, p)
    ax.plot(lin_x, lut_perc)
    all_v.append(lin_x)
    all_perc.append(lut_perc)

all_v = np.concatenate(all_v)
all_perc = np.concatenate(all_perc)

p0 = 0.0
p3 = 1.0
def piecewise_linear(v, v1, v2, p1, p2):
    condlist = [v < v1, (v >= v1) & (v < v2), v >= v2]
    funclist = [lambda v: (p1-p0)*(v-v0)/(v1-v0) + p0, lambda v: (p2-p1)*(v-v1)/(v2-v1) + p1, lambda v: (p3-p2)*(v-v2)/(v3-v2) + p2]
    return np.piecewise(v, condlist, funclist)

popt, pcov = curve_fit(piecewise_linear, all_v, all_perc, p0=[(v3-v0)*0.3+v0, (v3-v0)*0.6+v0, 1.0, 1.0])
plt.plot(lin_x, piecewise_linear(lin_x, *popt))

names = ["V1", "V2", "P1", "P2"]
consts = {}
for p, n in zip(popt, names):
    consts[n] = p

consts["V0"] = v0
consts["V3"] = v3
consts["P0"] = p0
consts["P3"] = p3

print(consts)

for n, p in sorted(consts.items()):
    print(f"const {n}: f32 = {p};")

plt.show()

