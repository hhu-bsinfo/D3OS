## =============================================================================
## FILE        : benchmark.rs
## AUTHOR      : Johann Spenrath <johann.spenrath@hhu.de>
## DESCRIPTION : functions for sending and receiving packets and printing stats
## =============================================================================
## NOTES:
## =============================================================================
## DEPENDENCIES:
## =============================================================================
import socket
import sys
from datetime import datetime, timedelta
import time


packet_count = 0
#buffer size 
buffer_size = 4096000

packets_received = 0
packets_out_of_order = 0
duplicated_packets = 0
bytes_received = 0
current_packet_number = 0
previous_packet_number = 0
interval_counter = 0
bytes_received_in_interval = 0
bytes_received_total = 0



# get the arguments
if len(sys.argv) == 3:
    # Get "IP address of Server" and also the "port number" from argument 1 and argument 2
    ip = sys.argv[1]
    port = int(sys.argv[2])
else:
    print("Run like : python3 server.py <arg1:server ip:this system IP 192.168.1.6> <arg2:server port:4444 >")
    exit(1)

# Create a UDP socket
socket_handle = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)

# Bind the socket to the port
server_address = (ip, port)
socket_handle.bind(server_address)


print("Do Ctrl+c to exit the program !!")



print(f"## server is listening from {ip} on Port {port} ")
print(f"start: {datetime.now().time()}")
seconds_passed = int(time.time())
#seconds_passed = int(time.time() + 1)
#print("\n\n 2. Server received: ", data.decode('utf-8'), "\n\n")
while True:
    # extract data payload and address from where the packet was sent
    #try:
    data, address = socket_handle.recvfrom(buffer_size) 
    if data:
        packets_received += 1
        # If we got a packet
    #except OSError as e:
    #    print(f"nettest: Failed to receive echo request! ({e})")
    #    break
    
    if packets_received == 2000:
        break
    
    bytes_received_in_interval = bytes_received_in_interval + len(data)
    bytes_received_total += len(data)

    while seconds_passed < int(time.time()):
        # One or more whole seconds have elapsed; print for each missed second.
        print(f"{interval_counter}-{interval_counter + 1}:    {bytes_received_in_interval / 1000:.0f} KB/s", flush=True)
        interval_counter += 1
        # Reset interval bytes *after* reporting
        bytes_received_in_interval = 0
        # Advance our "secondsPassed" marker by one second
        seconds_passed += 1

    #if seconds_passed < int(time.time()):
    #    print(f"{interval_counter} - {interval_counter + 1}: {bytes_received_in_interval/1000} KB/s")
    #    interval_counter += 1
    #    bytes_received = bytes_received + bytes_received_in_interval
    #    bytes_received_in_interval = 0
    #    seconds_passed += 1 

bytes_received = bytes_received + bytes_received_in_interval
duration_s = max(1, interval_counter)  # avoid division by zero
avg_kbps = (bytes_received_total / 1000) / duration_s

#print(f"{interval_counter} - {interval_counter + 1}: {bytes_received_in_interval/1000} KB/s")
print(f"------------------------------------------------------------------------")
print(f"Number of packets received : {packets_received}")
print(f"Total bytes received       :   {bytes_received_total}")
print(f"Bytes received             : {bytes_received / 1000} KB/s")
print(f"Bytes received             : {bytes_received } B/s")
print(f"Average Bytes received     : {(bytes_received / (interval_counter+1)) / 1000} KB/s")
print(f"packets out of order       : {packets_out_of_order} / {packets_received}")
print(f"duplicated packets         : {duplicated_packets}")
print(f"duration : {duration_s}")
print(f"Average throughput:     {avg_kbps:.1f} KB/s")
#print(f"Packet #{packet_count} from {address}: {data.decode(errors='ignore')}")
print(f"------------------------------------------------------------------------")





    #print(" payload size ", data.len())
    #send_data = input("Type some text to send => ")
    #s.sendto(send_data.encode('utf-8'), address)
    #print("\n\n 1. Server sent : ", send_data,"\n\n")
