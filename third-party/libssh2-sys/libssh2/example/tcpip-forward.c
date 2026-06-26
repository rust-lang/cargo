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

/* resolved by the server */
static const char *remote_listenhost = "localhost";

static int remote_wantport = 2222;
static int remote_listenport;

static const char *local_destip = "127.0.0.1";
static int local_destport = 22;

enum {
    AUTH_NONE = 0,
    AUTH_PASSWORD = 1,
    AUTH_PUBLICKEY = 2
};

int main(int argc, char *argv[])
{
    int i, auth = AUTH_NONE;
    struct sockaddr_in sin;
    socklen_t sinlen = sizeof(sin);
    const char *fingerprint;
    char *userauthlist;
    int rc;
    LIBSSH2_SESSION *session = NULL;
    LIBSSH2_LISTENER *listener = NULL;
    LIBSSH2_CHANNEL *channel = NULL;
    struct timeval tv;
    ssize_t len, wr;
    char buf[16384];
    libssh2_socket_t sock;
    libssh2_socket_t forwardsock = LIBSSH2_INVALID_SOCKET;

#ifdef _WIN32
    WSADATA wsadata;

    rc = WSAStartup(MAKEWORD(2, 0), &wsadata);
    if(rc) {
        fprintf(stderr, "WSAStartup failed with error: %d\n", rc);
        return 1;
    }
#endif

    if(argc > 1)
        server_ip = argv[1];
    if(argc > 2)
        username = argv[2];
    if(argc > 3)
        password = argv[3];
    if(argc > 4)
        remote_listenhost = argv[4];
    if(argc > 5)
        remote_wantport = atoi(argv[5]);
    if(argc > 6)
        local_destip = argv[6];
    if(argc > 7)
        local_destport = atoi(argv[7]);

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

    fprintf(stderr, "Asking server to listen on remote %s:%d\n",
            remote_listenhost, remote_wantport);

    listener = libssh2_channel_forward_listen_ex(session, remote_listenhost,
                                                 remote_wantport,
                                                 &remote_listenport, 1);
    if(!listener) {
        fprintf(stderr, "Could not start the tcpip-forward listener.\n"
                        "(Note that this can be a problem at the server."
                        " Please review the server logs.)\n");
        goto shutdown;
    }

    fprintf(stderr, "Server is listening on %s:%d\n", remote_listenhost,
            remote_listenport);

    fprintf(stderr, "Waiting for remote connection\n");
    channel = libssh2_channel_forward_accept(listener);
    if(!channel) {
        fprintf(stderr, "Could not accept connection.\n"
                        "(Note that this can be a problem at the server."
                        " Please review the server logs.)\n");
        goto shutdown;
    }

    fprintf(stderr,
            "Accepted remote connection. Connecting to local server %s:%d\n",
            local_destip, local_destport);
    forwardsock = socket(PF_INET, SOCK_STREAM, IPPROTO_TCP);
    if(forwardsock == LIBSSH2_INVALID_SOCKET) {
        fprintf(stderr, "failed to open forward socket.\n");
        goto shutdown;
    }

    sin.sin_family = AF_INET;
    sin.sin_port = htons((unsigned short)local_destport);
    sin.sin_addr.s_addr = inet_addr(local_destip);
    if(INADDR_NONE == sin.sin_addr.s_addr) {
        fprintf(stderr, "failed in inet_addr().\n");
        goto shutdown;
    }
    if(-1 == connect(forwardsock, (struct sockaddr *)&sin, sinlen)) {
        fprintf(stderr, "failed to connect().\n");
        goto shutdown;
    }

    fprintf(stderr, "Forwarding connection from remote %s:%d to local %s:%d\n",
            remote_listenhost, remote_listenport,
            local_destip, local_destport);

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
            ssize_t nwritten;
            len = recv(forwardsock, buf, sizeof(buf), 0);
            if(len < 0) {
                fprintf(stderr, "failed to recv().\n");
                goto shutdown;
            }
            else if(len == 0) {
                fprintf(stderr, "The local server at %s:%d disconnected.\n",
                        local_destip, local_destport);
                goto shutdown;
            }
            wr = 0;
            do {
                nwritten = libssh2_channel_write(channel, buf, (size_t)len);
                if(nwritten < 0) {
                    fprintf(stderr, "libssh2_channel_write: %ld\n",
                            (long)nwritten);
                    goto shutdown;
                }
                wr += nwritten;
            } while(nwritten > 0 && wr < len);
        }
        for(;;) {
            ssize_t nsent;
            len = libssh2_channel_read(channel, buf, sizeof(buf));
            if(LIBSSH2_ERROR_EAGAIN == len)
                break;
            else if(len < 0) {
                fprintf(stderr, "libssh2_channel_read: %ld",
                        (long)len);
                goto shutdown;
            }
            wr = 0;
            while(wr < len) {
                nsent = send(forwardsock, buf + wr, (size_t)(len - wr), 0);
                if(nsent <= 0) {
                    fprintf(stderr, "failed to send().\n");
                    goto shutdown;
                }
                wr += nsent;
            }
            if(libssh2_channel_eof(channel)) {
                fprintf(stderr, "The remote client at %s:%d disconnected.\n",
                        remote_listenhost, remote_listenport);
                goto shutdown;
            }
        }
    }

shutdown:

    if(forwardsock != LIBSSH2_INVALID_SOCKET) {
        shutdown(forwardsock, 2);
        LIBSSH2_SOCKET_CLOSE(forwardsock);
    }

    if(channel)
        libssh2_channel_free(channel);

    if(listener)
        libssh2_channel_forward_cancel(listener);

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
