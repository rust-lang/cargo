/* Copyright (C) The libssh2 project and its contributors.
 *
 * Sample showing how to use libssh2 to request agent forwarding
 * on the remote host. The command executed will run with agent forwarded
 * so you should be able to do things like clone out protected git
 * repos and such.
 *
 * The example uses agent authentication to ensure an agent to forward
 * is running.
 *
 * $ ./ssh2_agent_forwarding 127.0.0.1 user "uptime"
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

static const char *hostname = "127.0.0.1";
static const char *commandline = "uptime";
static const char *username;

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

int main(int argc, char *argv[])
{
    uint32_t hostaddr;
    libssh2_socket_t sock;
    struct sockaddr_in sin;
    int rc;
    LIBSSH2_SESSION *session = NULL;
    LIBSSH2_CHANNEL *channel;
    LIBSSH2_AGENT *agent = NULL;
    struct libssh2_agent_publickey *identity, *prev_identity = NULL;
    int exitcode;
    char *exitsignal = (char *)"none";
    ssize_t bytecount = 0;

#ifdef _WIN32
    WSADATA wsadata;

    rc = WSAStartup(MAKEWORD(2, 0), &wsadata);
    if(rc) {
        fprintf(stderr, "WSAStartup failed with error: %d\n", rc);
        return 1;
    }
#endif

    if(argc < 2) {
        fprintf(stderr, "At least IP and username arguments are required.\n");
        return 1;
    }
    /* must be ip address only */
    hostname = argv[1];
    username = argv[2];

    if(argc > 3) {
        commandline = argv[3];
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

    if(libssh2_session_handshake(session, sock) != 0) {
        fprintf(stderr, "Failure establishing SSH session: %d\n", rc);
        goto shutdown;
    }

    /* Connect to the ssh-agent */
    agent = libssh2_agent_init(session);
    if(!agent) {
        fprintf(stderr, "Failure initializing ssh-agent support\n");
        rc = 1;
        goto shutdown;
    }
    if(libssh2_agent_connect(agent)) {
        fprintf(stderr, "Failure connecting to ssh-agent\n");
        rc = 1;
        goto shutdown;
    }
    if(libssh2_agent_list_identities(agent)) {
        fprintf(stderr, "Failure requesting identities to ssh-agent\n");
        rc = 1;
        goto shutdown;
    }
    for(;;) {
        rc = libssh2_agent_get_identity(agent, &identity, prev_identity);
        if(rc == 1)
            break;
        if(rc < 0) {
            fprintf(stderr,
                    "Failure obtaining identity from ssh-agent support\n");
            rc = 1;
            goto shutdown;
        }
        if(libssh2_agent_userauth(agent, username, identity)) {
            fprintf(stderr, "Authentication with username %s and "
                    "public key %s failed.\n",
                    username, identity->comment);
        }
        else {
            fprintf(stderr, "Authentication with username %s and "
                    "public key %s succeeded.\n",
                    username, identity->comment);
            break;
        }
        prev_identity = identity;
    }
    if(rc) {
        fprintf(stderr, "Could not continue authentication\n");
        goto shutdown;
    }

#if 0
    libssh2_trace(session, ~0);
#endif

    /* Set session to non-blocking */
    libssh2_session_set_blocking(session, 0);

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
    while((rc = libssh2_channel_request_auth_agent(channel)) ==
          LIBSSH2_ERROR_EAGAIN) {
        waitsocket(sock, session);
    }
    if(rc) {
        fprintf(stderr, "Error, could not request auth agent, "
                "error code %d.\n", rc);
        exit(1);
    }
    else {
        fprintf(stdout, "Agent forwarding request succeeded.\n");
    }
    while((rc = libssh2_channel_exec(channel, commandline)) ==
          LIBSSH2_ERROR_EAGAIN) {
        waitsocket(sock, session);
    }
    if(rc) {
        fprintf(stderr, "Error\n");
        exit(1);
    }
    for(;;) {
        ssize_t nread;
        /* loop until we block */
        do {
            char buffer[0x4000];
            nread = libssh2_channel_read(channel, buffer, sizeof(buffer) );
            if(nread > 0) {
                ssize_t i;
                bytecount += nread;
                fprintf(stderr, "We read:\n");
                for(i = 0; i < nread; ++i)
                    fputc(buffer[i], stderr);
                fprintf(stderr, "\n");
            }
            else {
                if(nread != LIBSSH2_ERROR_EAGAIN)
                    /* no need to output this for the EAGAIN case */
                    fprintf(stderr, "libssh2_channel_read returned %ld\n",
                            (long)nread);
            }
        } while(nread > 0);

        /* this is due to blocking that would occur otherwise so we loop on
           this condition */
        if(nread == LIBSSH2_ERROR_EAGAIN) {
            waitsocket(sock, session);
        }
        else
            break;
    }
    exitcode = 127;
    while((rc = libssh2_channel_close(channel)) == LIBSSH2_ERROR_EAGAIN) {
        waitsocket(sock, session);
    }
    if(rc == 0) {
        exitcode = libssh2_channel_get_exit_status(channel);
        libssh2_channel_get_exit_signal(channel, &exitsignal,
                                        NULL, NULL, NULL, NULL, NULL);
    }

    if(exitsignal) {
        fprintf(stderr, "\nGot signal: %s\n", exitsignal);
    }
    else {
        fprintf(stderr, "\nEXIT: %d bytecount: %ld\n",
                exitcode, (long)bytecount);
    }

    libssh2_channel_free(channel);
    channel = NULL;

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

    return 0;
}
