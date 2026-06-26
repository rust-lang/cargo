/* Copyright (C) The libssh2 project and its contributors.
 *
 * The code sends a 'cat' command, and then writes a lot of data to it only to
 * check that reading the returned data sums up to the same amount.
 *
 * $ ./ssh2_echo 127.0.0.1 user password
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

#include "libssh2_setup.h"
#include <libssh2.h>

#ifdef HAVE_SYS_SOCKET_H
#include <sys/socket.h>
#endif
#ifdef HAVE_UNISTD_H
#include <unistd.h>
#endif
#ifdef HAVE_NETINET_IN_H
#include <netinet/in.h>
#endif
#ifdef HAVE_ARPA_INET_H
#include <arpa/inet.h>
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static const char *hostname = "127.0.0.1";
static const char *commandline = "cat";
static const char *username = "user";
static const char *password = "password";

static int waitsocket(libssh2_socket_t socket_fd, LIBSSH2_SESSION *session)
{
    struct timeval timeout;
    int rc;
    fd_set fd;
    fd_set *writefd = NULL;
    fd_set *readfd = NULL;
    int dir;

    timeout.tv_sec = 10;
    timeout.tv_usec = 0;

    FD_ZERO(&fd);

#if defined(__GNUC__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wsign-conversion"
#endif
    FD_SET(socket_fd, &fd);
#if defined(__GNUC__)
#pragma GCC diagnostic pop
#endif

    /* now make sure we wait in the correct direction */
    dir = libssh2_session_block_directions(session);

    if(dir & LIBSSH2_SESSION_BLOCK_INBOUND)
        readfd = &fd;

    if(dir & LIBSSH2_SESSION_BLOCK_OUTBOUND)
        writefd = &fd;

    rc = select((int)(socket_fd + 1), readfd, writefd, NULL, &timeout);

    return rc;
}

#define BUFSIZE 32000

int main(int argc, char *argv[])
{
    uint32_t hostaddr;
    libssh2_socket_t sock;
    struct sockaddr_in sin;
    const char *fingerprint;
    int rc;
    LIBSSH2_SESSION *session = NULL;
    LIBSSH2_CHANNEL *channel;
    int exitcode = 0;
    char *exitsignal = (char *)"none";
    size_t len;
    LIBSSH2_KNOWNHOSTS *nh;
    int type;

#ifdef _WIN32
    WSADATA wsadata;

    rc = WSAStartup(MAKEWORD(2, 0), &wsadata);
    if(rc) {
        fprintf(stderr, "WSAStartup failed with error: %d\n", rc);
        return 1;
    }
#endif

    if(argc > 1) {
        hostname = argv[1];  /* must be ip address only */
    }
    if(argc > 2) {
        username = argv[2];
    }
    if(argc > 3) {
        password = argv[3];
    }

    rc = libssh2_init(0);
    if(rc) {
        fprintf(stderr, "libssh2 initialization failed (%d)\n", rc);
        return 1;
    }

    hostaddr = inet_addr(hostname);

    /* Ultra basic "connect to port 22 on localhost".  Your code is
     * responsible for creating the socket establishing the connection
     */
    sock = socket(AF_INET, SOCK_STREAM, 0);
    if(sock == LIBSSH2_INVALID_SOCKET) {
        fprintf(stderr, "failed to create socket.\n");
        goto shutdown;
    }

    sin.sin_family = AF_INET;
    sin.sin_port = htons(22);
    sin.sin_addr.s_addr = hostaddr;
    if(connect(sock, (struct sockaddr*)(&sin), sizeof(struct sockaddr_in))) {
        fprintf(stderr, "failed to connect.\n");
        goto shutdown;
    }

    /* Create a session instance */
    session = libssh2_session_init();
    if(!session) {
        fprintf(stderr, "Could not initialize SSH session.\n");
        goto shutdown;
    }

    /* tell libssh2 we want it all done non-blocking */
    libssh2_session_set_blocking(session, 0);

    /* ... start it up. This will trade welcome banners, exchange keys,
     * and setup crypto, compression, and MAC layers
     */
    while((rc = libssh2_session_handshake(session, sock)) ==
          LIBSSH2_ERROR_EAGAIN);
    if(rc) {
        fprintf(stderr, "Failure establishing SSH session: %d\n", rc);
        goto shutdown;
    }

    nh = libssh2_knownhost_init(session);
    if(!nh) {
        /* eeek, do cleanup here */
        return 2;
    }

    /* read all hosts from here */
    libssh2_knownhost_readfile(nh, "known_hosts",
                               LIBSSH2_KNOWNHOST_FILE_OPENSSH);

    /* store all known hosts to here */
    libssh2_knownhost_writefile(nh, "dumpfile",
                                LIBSSH2_KNOWNHOST_FILE_OPENSSH);

    fingerprint = libssh2_session_hostkey(session, &len, &type);
    if(fingerprint) {
        struct libssh2_knownhost *host;
        int check = libssh2_knownhost_checkp(nh, hostname, 22,
                                             fingerprint, len,
                                             LIBSSH2_KNOWNHOST_TYPE_PLAIN|
                                             LIBSSH2_KNOWNHOST_KEYENC_RAW,
                                             &host);

        fprintf(stderr, "Host check: %d, key: %s\n", check,
                (check <= LIBSSH2_KNOWNHOST_CHECK_MISMATCH) ?
                host->key : "<none>");

        /*****
         * At this point, we could verify that 'check' tells us the key is
         * fine or bail out.
         *****/
    }
    else {
        /* eeek, do cleanup here */
        return 3;
    }
    libssh2_knownhost_free(nh);

    if(strlen(password) != 0) {
        /* We could authenticate via password */
        while((rc = libssh2_userauth_password(session, username, password)) ==
              LIBSSH2_ERROR_EAGAIN);
        if(rc) {
            fprintf(stderr, "Authentication by password failed.\n");
            exit(1);
        }
    }

    libssh2_trace(session, LIBSSH2_TRACE_SOCKET);

    /* Exec non-blocking on the remote host */
    do {
        channel = libssh2_channel_open_session(session);
        if(channel ||
           libssh2_session_last_error(session, NULL, NULL, 0) !=
           LIBSSH2_ERROR_EAGAIN)
            break;
        waitsocket(sock, session);
    } while(1);
    if(!channel) {
        fprintf(stderr, "Error\n");
        exit(1);
    }
    while((rc = libssh2_channel_exec(channel, commandline)) ==
          LIBSSH2_ERROR_EAGAIN) {
        waitsocket(sock, session);
    }
    if(rc) {
        fprintf(stderr, "exec error\n");
        exit(1);
    }
    else {
        LIBSSH2_POLLFD *fds = NULL;
        int running = 1;
        size_t bufsize = BUFSIZE;
        char buffer[BUFSIZE];
        size_t totsize = 1500000;
        size_t totwritten = 0;
        size_t totread = 0;
        int rereads = 0;
        int rewrites = 0;
        int i;

        for(i = 0; i < BUFSIZE; i++)
            buffer[i] = 'A';

        fds = malloc(sizeof(LIBSSH2_POLLFD));
        if(!fds) {
            fprintf(stderr, "malloc failed\n");
            exit(1);
        }

        fds[0].type = LIBSSH2_POLLFD_CHANNEL;
        fds[0].fd.channel = channel;
        fds[0].events = LIBSSH2_POLLFD_POLLIN | LIBSSH2_POLLFD_POLLOUT;

        do {
            int act = 0;
            rc = libssh2_poll(fds, 1, 10);

            if(rc < 1)
                continue;

            if(fds[0].revents & LIBSSH2_POLLFD_POLLIN) {
                ssize_t n = libssh2_channel_read(channel,
                                                 buffer, sizeof(buffer));
                act++;

                if(n == LIBSSH2_ERROR_EAGAIN) {
                    rereads++;
                    fprintf(stderr, "will read again\n");
                }
                else if(n < 0) {
                    fprintf(stderr, "read failed\n");
                    exit(1);
                }
                else {
                    totread += (size_t)n;
                    fprintf(stderr, "read %ld bytes (%lu in total)\n",
                            (long)n, (unsigned long)totread);
                }
            }

            if(fds[0].revents & LIBSSH2_POLLFD_POLLOUT) {
                act++;

                if(totwritten < totsize) {
                    /* we have not written all data yet */
                    size_t left = totsize - totwritten;
                    size_t size = (left < bufsize) ? left : bufsize;
                    ssize_t n = libssh2_channel_write_ex(channel, 0,
                                                         buffer, size);

                    if(n == LIBSSH2_ERROR_EAGAIN) {
                        rewrites++;
                        fprintf(stderr, "will write again\n");
                    }
                    else if(n < 0) {
                        fprintf(stderr, "write failed\n");
                        exit(1);
                    }
                    else {
                        totwritten += (size_t)n;
                        fprintf(stderr, "wrote %ld bytes (%lu in total)",
                                (long)n, (unsigned long)totwritten);
                        if(left >= bufsize && (size_t)n != bufsize) {
                            fprintf(stderr, " PARTIAL");
                        }
                        fprintf(stderr, "\n");
                    }
                }
                else {
                    /* all data written, send EOF */
                    rc = libssh2_channel_send_eof(channel);

                    if(rc == LIBSSH2_ERROR_EAGAIN) {
                        fprintf(stderr, "will send eof again\n");
                    }
                    else if(rc < 0) {
                        fprintf(stderr, "send eof failed\n");
                        exit(1);
                    }
                    else {
                        fprintf(stderr, "sent eof\n");
                        /* we're done writing, stop listening for OUT events */
                        fds[0].events &=
                            ~(unsigned long)LIBSSH2_POLLFD_POLLOUT;
                    }
                }
            }

            if(fds[0].revents & LIBSSH2_POLLFD_CHANNEL_CLOSED) {
                if(!act) /* don't leave loop until we have read all data */
                    running = 0;
            }
        } while(running);

        exitcode = 127;
        while((rc = libssh2_channel_close(channel)) == LIBSSH2_ERROR_EAGAIN)
            waitsocket(sock, session);

        if(rc == 0) {
            exitcode = libssh2_channel_get_exit_status(channel);
            libssh2_channel_get_exit_signal(channel, &exitsignal,
                                            NULL, NULL, NULL, NULL, NULL);
        }

        if(exitsignal)
            fprintf(stderr, "\nGot signal: %s\n", exitsignal);

        libssh2_channel_free(channel);
        channel = NULL;

        fprintf(stderr, "\nrereads: %d rewrites: %d totwritten %lu\n",
                rereads, rewrites, (unsigned long)totwritten);

        if(totwritten != totread) {
            fprintf(stderr, "\n*** FAIL bytes written: "
                    "%lu bytes read: %lu ***\n",
                    (unsigned long)totwritten, (unsigned long)totread);
            exit(1);
        }
    }

shutdown:

    if(session) {
        libssh2_session_disconnect(session, "Normal Shutdown");
        libssh2_session_free(session);
    }

    if(sock != LIBSSH2_INVALID_SOCKET) {
        shutdown(sock, 2);
        LIBSSH2_SOCKET_CLOSE(sock);
    }

    fprintf(stderr, "all done\n");

    libssh2_exit();

#ifdef _WIN32
    WSACleanup();
#endif

    return exitcode;
}
