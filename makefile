socket: socket.c
	clang -Wall -Wextra -o ./build/socket socket.c

client: client.c
	clang -Wall -Wextra -o ./build/client client.c

.PHONY: runsocket
runsocket: socket
	./build/socket

.PHONY: runclient
runclient: client
	./build/client
