#include <stdio.h>
#include <sys/socket.h>
#include <string.h>
#include <arpa/inet.h>
#include <unistd.h>

int main(void) {
    // AF_INET => IPv4 SOCK_STREAM => 字节流 0 => 让系统选择默认协议
    int fd = socket(AF_INET, SOCK_STREAM, 0); // an ipv4 tcp socket

    if (fd == -1) {
        perror("socket");
        return 1;
    }

    /*
     * struct sockaddr_in {
     *     uint8_t         sin_len;
     *     sa_family_t     sin_family;
     *     in_port_t       sin_port;
     *     struct in_addr  sin_addr;
     *     char            sin_zero[8];
     * };
     */

    /*
     * struct in_addr {
     *     in_addr_t s_addr;
     * };
     */

    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));

    addr.sin_family = AF_INET;
    addr.sin_port   = htons(9000);

    int ret = inet_pton(AF_INET, "127.0.0.1", &addr.sin_addr);

    if (ret == 0) {
        fprintf(stderr, "invalid IP address\n");
        close(fd);
        return 1;
    }

    if (ret == -1) {
        perror("inet_pton");
        close(fd);
        return 1;
    }

    ret = bind(fd, (struct sockaddr *)&addr, sizeof(addr));

    if (ret == -1) {
        perror("bind");
        close(fd);
        return 1;
    }

    ret = listen(fd, 5);

    if (ret == -1) {
        perror("listen");
        close(fd);
        return 1;
    }

    while(1) {
        int client_fd = accept(fd, NULL, NULL);

        if (client_fd == -1) {
            perror("accept");
            continue;
        }


        char buf[1024];

        while(1) {
            ssize_t n = recv(client_fd, buf, sizeof(buf) - 1, 0);

            if (n > 0) {
                buf[n] = '\0';
                printf("recv %zd bytes: %s\n", n, buf);

                ssize_t total = 0;
                while (total < n) {
                    ssize_t sent = send(client_fd, buf + total, n - total, 0);

                    if (sent == -1) {
                        perror("send");
                        break;
                    }
                    total += sent;
                }
            } else if (n == 0) {
                printf("client closed connection\n");
                break;
            } else {
                perror("recv");
                break;
            }
        }
        close(client_fd);
    }
}
