/* Copyright (C) The libssh2 project and its contributors.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

#include "libssh2_setup.h"
#include <libssh2.h>

#ifdef _WIN32
#include <ws2tcpip.h>  /* for socklen_t */
#define recv(s, b, l, f)  recv((s), (b), (int)(l), (f))
#define send(s, b, l, f)  send((s), (b), (int)(l), (f))
#endif

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

#ifndef INADDR_NONE
#define INADDR_NONE (in_addr_t)~0
#endif

static const char *pubkey = "/home/username/.ssh/id_rsa.pub";
static const char *privkey = "/home/username/.ssh/id_rsa";
static const char *username = "username";
static const char *password = "";

static const char *server_ip = "127.0.0.1";

static const char *local_listenip = "127.0.0.1";
static int local_listenport = 2222;

static const char *remote_desthost = "localhost"; /* resolved by the server */
static int remote_destport = 22;

enum {
    AUTH_NONE = 0,
    AUTH_PASSWORD = 1,
    AUTH_PUBLICKEY = 2
};

int main(int argc, char *argv[])
{
    int i, auth = AUTH_NONE;
    struct sockaddr_in sin;
    socklen_t sinlen;
    const char *fingerprint;
    char *userauthlist;
    int rc;
    LIBSSH2_SESSION *session = NULL;
    LIBSSH2_CHANNEL *channel = NULL;
    const char *shost;
    int sport;
    struct timeval tv;
    ssize_t len, wr;
    char buf[16384];
    libssh2_socket_t sock;
    libssh2_socket_t listensock = LIBSSH2_INVALID_SOCKET;
    libssh2_socket_t forwardsock = LIBSSH2_INVALID_SOCKET;

#ifdef _WIN32
    char sockopt;
    WSADATA wsadata;

    rc = WSAStartup(MAKEWORD(2, 0), &wsadata);
    if(rc) {
        fprintf(stderr, "WSAStartup failed with error: %d\n", rc);
        return 1;
    }
#else
    int sockopt;
#endif

    if(argc > 1)
        server_ip = argv[1];
    if(argc > 2)
        username = argv[2];
    if(argc > 3)
        password = argv[3];
    if(argc > 4)
        local_listenip = argv[4];
    if(argc > 5)
        local_listenport = atoi(argv[5]);
    if(argc > 6)
        remote_desthost = argv[6];
    if(argc > 7)
        remote_destport = atoi(argv[7]);

    rc = libssh2_init(0);
    if(rc) {
        fprintf(stderr, "libssh2 initialization failed (%d)\n", rc);
        return 1;
    }

    /* Connect to SSH server */
    sock = socket(PF_INET, SOCK_STREAM, IPPROTO_TCP);
    if(sock == LIBSSH2_INVALID_SOCKET) {
        fprintf(stderr, "failed to open socket.\n");
        goto shutdown;
    }

    sin.sin_family = AF_INET;
    sin.sin_addr.s_addr = inet_addr(server_ip);
    if(INADDR_NONE == sin.sin_addr.s_addr) {
        fprintf(stderr, "inet_addr: Invalid IP address '%s'\n", server_ip);
        goto shutdown;
    }
    sin.sin_port = htons(22);
    if(connect(sock, (struct sockaddr*)(&sin), sizeof(struct sockaddr_in))) {
        fprintf(stderr, "Failed to connect to %s.\n", inet_ntoa(sin.sin_addr));
        goto shutdown;
    }

    /* Create a session instance */
    session = libssh2_session_init();
    if(!session) {
        fprintf(stderr, "Could not initialize SSH session.\n");
        goto shutdown;
    }

    /* ... start it up. This will trade welcome banners, exchange keys,
     * and setup crypto, compression, and MAC layers
     */
    rc = libssh2_session_handshake(session, sock);
    if(rc) {
        fprintf(stderr, "Error when starting up SSH session: %d\n", rc);
        goto shutdown;
    }

    /* At this point we have not yet authenticated.  The first thing to do
     * is check the hostkey's fingerprint against our known hosts Your app
     * may have it hard coded, may go to a file, may present it to the
     * user, that's your call
     */
    fingerprint = libssh2_hostkey_hash(session, LIBSSH2_HOSTKEY_HASH_SHA1);
    fprintf(stderr, "Fingerprint: ");
    for(i = 0; i < 20; i++)
        fprintf(stderr, "%02X ", (unsigned char)fingerprint[i]);
    fprintf(stderr, "\n");

    /* check what authentication methods are available */
    userauthlist = libssh2_userauth_list(session, username,
                                         (unsigned int)strlen(username));
    if(userauthlist) {
        fprintf(stderr, "Authentication methods: %s\n", userauthlist);
        if(strstr(userauthlist, "password"))
            auth |= AUTH_PASSWORD;
        if(strstr(userauthlist, "publickey"))
            auth |= AUTH_PUBLICKEY;

        /* check for options */
        if(argc > 8) {
            if((auth & AUTH_PASSWORD) && !strcmp(argv[8], "-p"))
                auth = AUTH_PASSWORD;
            if((auth & AUTH_PUBLICKEY) && !strcmp(argv[8], "-k"))
                auth = AUTH_PUBLICKEY;
        }

        if(auth & AUTH_PASSWORD) {
            if(libssh2_userauth_password(session, username, password)) {
                fprintf(stderr, "Authentication by password failed.\n");
                goto shutdown;
            }
        }
        else if(auth & AUTH_PUBLICKEY) {
            if(libssh2_userauth_publickey_fromfile(session, username,
                                                   pubkey, privkey,
                                                   password)) {
                fprintf(stderr, "Authentication by public key failed.\n");
                goto shutdown;
            }
            else {
                fprintf(stderr, "Authentication by public key succeeded.\n");
            }
        }
        else {
            fprintf(stderr, "No supported authentication methods found.\n");
            goto shutdown;
        }
    }

    listensock = socket(PF_INET, SOCK_STREAM, IPPROTO_TCP);
    if(listensock == LIBSSH2_INVALID_SOCKET) {
        fprintf(stderr, "failed to open listen socket.\n");
        goto shutdown;
    }

    sin.sin_family = AF_INET;
    sin.sin_port = htons((unsigned short)local_listenport);
    sin.sin_addr.s_addr = inet_addr(local_listenip);
    if(INADDR_NONE == sin.sin_addr.s_addr) {
        fprintf(stderr, "failed in inet_addr().\n");
        goto shutdown;
    }
    sockopt = 1;
    setsockopt(listensock, SOL_SOCKET, SO_REUSEADDR, &sockopt,
               sizeof(sockopt));
    sinlen = sizeof(sin);
    if(-1 == bind(listensock, (struct sockaddr *)&sin, sinlen)) {
        fprintf(stderr, "failed to bind().\n");
        goto shutdown;
    }
    if(-1 == listen(listensock, 2)) {
        fprintf(stderr, "failed to listen().\n");
        goto shutdown;
    }

    fprintf(stderr, "Waiting for TCP connection on %s:%d...\n",
            inet_ntoa(sin.sin_addr), ntohs(sin.sin_port));

    forwardsock = accept(listensock, (struct sockaddr *)&sin, &sinlen);
    if(forwardsock == LIBSSH2_INVALID_SOCKET) {
        fprintf(stderr, "failed to accept forward socket.\n");
        goto shutdown;
    }

    shost = inet_ntoa(sin.sin_addr);
    sport = ntohs(sin.sin_port);

    fprintf(stderr, "Forwarding connection from %s:%d here to remote %s:%d\n",
            shost, sport, remote_desthost, remote_destport);

    channel = libssh2_channel_direct_tcpip_ex(session, remote_desthost,
                                              remote_destport, shost, sport);
    if(!channel) {
        fprintf(stderr, "Could not open the direct-tcpip channel.\n"
                        "(Note that this can be a problem at the server."
                        " Please review the server logs.)\n");
        goto shutdown;
    }

    /* Must use non-blocking IO hereafter due to the current libssh2 API */
    libssh2_session_set_blocking(session, 0);

    for(;;) {
        fd_set fds;
        FD_ZERO(&fds);
#if defined(__GNUC__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wsign-conversion"
#endif
        FD_SET(forwardsock, &fds);
#if defined(__GNUC__)
#pragma GCC diagnostic pop
#endif
        tv.tv_sec = 0;
        tv.tv_usec = 100000;
        rc = select((int)(forwardsock + 1), &fds, NULL, NULL, &tv);
        if(-1 == rc) {
            fprintf(stderr, "failed to select().\n");
            goto shutdown;
        }
#if defined(__GNUC__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wsign-conversion"
#endif
        if(rc && FD_ISSET(forwardsock, &fds)) {
#if defined(__GNUC__)
#pragma GCC diagnostic pop
#endif
            len = recv(forwardsock, buf, sizeof(buf), 0);
            if(len < 0) {
                fprintf(stderr, "failed to recv().\n");
                goto shutdown;
            }
            else if(len == 0) {
                fprintf(stderr, "The client at %s:%d disconnected.\n", shost,
                        sport);
                goto shutdown;
            }
            wr = 0;
            while(wr < len) {
                ssize_t nwritten = libssh2_channel_write(channel,
                                                         buf + wr,
                                                         (size_t)(len - wr));
                if(nwritten == LIBSSH2_ERROR_EAGAIN) {
                    continue;
                }
                if(nwritten < 0) {
                    fprintf(stderr, "libssh2_channel_write: %ld\n",
                            (long)nwritten);
                    goto shutdown;
                }
                wr += nwritten;
            }
        }
        for(;;) {
            len = libssh2_channel_read(channel, buf, sizeof(buf));
            if(LIBSSH2_ERROR_EAGAIN == len)
                break;
            else if(len < 0) {
                fprintf(stderr, "libssh2_channel_read: %ld", (long)len);
                goto shutdown;
            }
            wr = 0;
            while(wr < len) {
                ssize_t nsent = send(forwardsock, buf + wr,
                                     (size_t)(len - wr), 0);
                if(nsent <= 0) {
                    fprintf(stderr, "failed to send().\n");
                    goto shutdown;
                }
                wr += nsent;
            }
            if(libssh2_channel_eof(channel)) {
                fprintf(stderr, "The server at %s:%d disconnected.\n",
                        remote_desthost, remote_destport);
                goto shutdown;
            }
        }
    }

shutdown:

    if(forwardsock != LIBSSH2_INVALID_SOCKET) {
        shutdown(forwardsock, 2);
        LIBSSH2_SOCKET_CLOSE(forwardsock);
    }

    if(listensock != LIBSSH2_INVALID_SOCKET) {
        shutdown(listensock, 2);
        LIBSSH2_SOCKET_CLOSE(listensock);
    }

    if(channel)
        libssh2_channel_free(channel);

    if(session) {
        libssh2_session_disconnect(session, "Normal Shutdown");
        libssh2_session_free(session);
    }

    if(sock != LIBSSH2_INVALID_SOCKET) {
        shutdown(sock, 2);
        LIBSSH2_SOCKET_CLOSE(sock);
    }

    libssh2_exit();

#ifdef _WIN32
    WSACleanup();
#endif

    return 0;
}
