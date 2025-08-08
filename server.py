import socket
import sys

if len(sys.argv) == 3:
    # Get "IP address of Server" and also the "port number" from argument 1 and argument 2
    ip = sys.argv[1]
    port = int(sys.argv[2])
else:
    print("Run like : python3 server.py <arg1:server ip:this system IP 192.168.1.6> <arg2:server port:4444 >")
    exit(1)
#buffer size 
buffer_size = 4096
# Create a UDP socket
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
# Bind the socket to the port
server_address = (ip, port)
s.bind(server_address)
print("Do Ctrl+c to exit the program !!")

packet_count = 0
print("####### Server is listening #######")
#print("\n\n 2. Server received: ", data.decode('utf-8'), "\n\n")
while True:
    # extract data payload and address from where the packet was sent
    data, address = s.recvfrom(buffer_size) 
    if data:  # If we got a packet
       packet_count += 1
       print(f"Packet #{packet_count} from {address}: {data.decode(errors='ignore')}")
    #print(" payload size ", data.len())
    #send_data = input("Type some text to send => ")
    #s.sendto(send_data.encode('utf-8'), address)
    #print("\n\n 1. Server sent : ", send_data,"\n\n")
