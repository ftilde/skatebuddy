#!/bin/python
import argparse
import matplotlib.pyplot as plt
import numpy as np
import torch
from torch import nn
from collections import OrderedDict

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

class Model(nn.Module):
    def __init__(self, dim):
        super().__init__()
        self.subs = nn.Parameter(torch.ones(dim, requires_grad=True, dtype=torch.float32))
        #self.mul = nn.Parameter(torch.ones(1, requires_grad=True, dtype=torch.float32))

    #def forward(self, x):
    #    x = x.unsqueeze(1).expand(-1, self.subs.shape[0])
    #    diff = x - self.subs
    #    prod = torch.prod(diff, dim=1) * self.mul
    #    return prod
    def forward(self, x):
        x = [x**i for i in range(0, self.subs.shape[0])]
        x = torch.cat(x, dim=1)
        ret = torch.sum(x * self.subs.unsqueeze(0), dim=1)
        return ret


def fit(voltages, percentages):
    voltages = voltages
    dim = 13

    #model = Model(dim)
    model = nn.Sequential(OrderedDict([
          ('lin1', nn.Linear(1, 5)),
          ('relu1', nn.ReLU()),
          ('lin2', nn.Linear(5, 5)),
          ('relu2', nn.ReLU()),
          ('lin5', nn.Linear(5, 1)),
        ]))


    # Define the loss function and the optimizer
    #loss_fn = nn.L1Loss()
    loss_fn = nn.MSELoss()
    learning_rate = 0.001
    optimizer = torch.optim.Adam(params = model.parameters(), lr = learning_rate)


    # Train the model
    epochs = 1000
    epoch_num = []
    train_losses = []
    test_losses = []
    batches = 100
    for epoch in range(epochs):
        #perm = np.random.permutation(len(voltages))
        #voltages = voltages[perm]
        #percentages = percentages[perm]
        for volt_batch, perc_batch in zip(np.array_split(voltages, batches), np.array_split(percentages, batches)):
            volt_batch = torch.from_numpy(volt_batch).unsqueeze(1)
            perc_batch = torch.from_numpy(perc_batch)
            model.train()
            perc_pred = model(volt_batch)
            loss = loss_fn(perc_batch, perc_pred)
            print(loss)
            optimizer.zero_grad()
            loss.backward()
            optimizer.step()

    return model


ax = plt.gca()
ax.set_xlabel("voltage")
ax.set_ylabel("percentage")

lin_x = np.linspace(v_0, v_100, 100, dtype=np.float32)
lin_y = [piecewise_linear(x) for x in lin_x]

all_voltages = []
all_percentages = []

for file in args.filename:
    print(file)
    d = np.loadtxt(file, delimiter=";", dtype=np.float32)
    time = d[:, 0]
    voltage = d[:, 1]
    percentage = 1.0-time/np.max(time)
    ax.plot(voltage, percentage)
    all_voltages.append(voltage)
    all_percentages.append(percentage)

all_voltages = np.concatenate(all_voltages)
all_percentages = np.concatenate(all_percentages)

model = fit(all_voltages, all_percentages)

fit_y = model(torch.from_numpy(lin_x).unsqueeze(1)).detach().numpy()
#fit_y = [model(torch.from_numpy(np.array([x]))).detach().numpy() for x in lin_x]
ax.plot(lin_x, fit_y)

plt.show()

