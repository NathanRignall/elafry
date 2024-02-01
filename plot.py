import csv
import matplotlib.pyplot as plt

# Read CSV file and extract data
heights = []
thrusts = []
setpoints = []
with open('output.csv', 'r') as csvfile:
    csvreader = csv.reader(csvfile)
    for row in csvreader:
        heights.append(float(row[1]))
        thrusts.append(float(row[2]))
        setpoints.append(float(row[3]))

# Plotting
plt.figure(figsize=(10, 6))
plt.plot(heights, label='Height (m)')
plt.plot(thrusts, label='Thrust (N)')
plt.plot(setpoints, label='Height Setpoint (m)')
plt.xlabel('Time Step')
plt.ylabel('Value')
plt.title('Height and Thrust vs. Time')
plt.legend()
plt.grid(True)
plt.show()
