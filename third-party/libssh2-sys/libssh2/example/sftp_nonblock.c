/* Copyright (C) The libssh2 project and its contributors.
 *
 * Sample showing how to do SFTP non-blocking transfers.
 *
 * The sample code has default values for host name, user name, password
 * and path to copy, but you can specify them on the command line like:
 *
 * $ ./sftp_nonblock 192.168.0.1 user password /tmp/secrets
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

#include "libssh2_setup.h"
#include <libssh2.h>
#include <libssh2_sftp.h>

#ifdef _WIN32
#define write(f, b, c)  write((f), (b), (unsigned int)(c))
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
#ifdef HAVE_SYS_TIME_H
#include <sys/time.h>
#endif

#include <stdio.h>

static const char *pubkey = "/home/username/.ssh/id_rsa.pub";
static const char *privkey = "/home/username/.ssh/id_rsa";
static const char *username = "username";
static const char *password = "password";
static const char *sftppath = "/tmp/TEST";

#ifdef HAVE_GETTIMEOFDAY
/* diff in ms */
static long tvdiff(struct timeval newer, struct timeval older)
{
    return (newer.tv_sec - older.tv_sec) * 1000 +
        (newer.tv_usec - older.tv_usec) / 1000;
}
#endif

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
    int i, auth_pw = 1;
    struct sockaddr_in sin;
    const char *fingerprint;
    int rc;
    LIBSSH2_SESSION *session = NULL;
    LIBSSH2_SFTP *sftp_session;
    LIBSSH2_SFTP_HANDLE *sftp_handle;
#ifdef HAVE_GETTIMEOFDAY
    struct timeval start;
    struct timeval end;
    long time_ms;
#endif
    libssh2_struct_stat_size total = 0;
    int spin = 0;

#ifdef _WIN32
    WSADATA wsadata;

    rc = WSAStartup(MAKEWORD(2, 0), &wsadata);
    if(rc) {
        fprintf(stderr, "WSAStartup failed with error: %d\n", rc);
        return 1;
    }
#endif

    if(argc > 1) {
        hostaddr = inet_addr(argv[1]);
    }
    else {
        hostaddr = htonl(0x7F000001);
    }
    if(argc > 2) {
        username = argv[2];
    }
    if(argc > 3) {
        password = argv[3];
    }
    if(argc > 4) {
        sftppath = argv[4];
    }

    rc = libssh2_init(0);
    if(rc) {
        fprintf(stderr, "libssh2 initialization failed (%d)\n", rc);
        return 1;
    }

    /*
     * The application code is responsible for creating the socket
     * and establishing the connection
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

    /* Since we have set non-blocking, tell libssh2 we are non-blocking */
    libssh2_session_set_blocking(session, 0);

#ifdef HAVE_GETTIMEOFDAY
    gettimeofday(&start, NULL);
#endif

    /* ... start it up. This will trade welcome banners, exchange keys,
     * and setup crypto, compression, and MAC layers
     */
    while((rc = libssh2_session_handshake(session, sock)) ==
          LIBSSH2_ERROR_EAGAIN);
    if(rc) {
        fprintf(stderr, "Failure establishing SSH session: %d\n", rc);
        goto shutdown;
    }

    /* At this point we have not yet authenticated.  The first thing to do
     * is check the hostkey's fingerprint against our known hosts Your app
     * may have it hard coded, may go to a file, may present it to the
     * user, that's your call
     */
    fingerprint = libssh2_hostkey_hash(session, LIBSSH2_HOSTKEY_HASH_SHA1);
    fprintf(stderr, "Fingerprint: ");
    for(i = 0; i < 20; i++) {
        fprintf(stderr, "%02X ", (unsigned char)fingerprint[i]);
    }
    fprintf(stderr, "\n");

    if(auth_pw) {
        /* We could authenticate via password */
        while((rc = libssh2_userauth_password(session, username, password)) ==
              LIBSSH2_ERROR_EAGAIN);
        if(rc) {
            fprintf(stderr, "Authentication by password failed.\n");
            goto shutdown;
        }
    }
    else {
        /* Or by public key */
        while((rc =
              libssh2_userauth_publickey_fromfile(session, username,
                                                  pubkey, privkey,
                                                  password)) ==
              LIBSSH2_ERROR_EAGAIN);
        if(rc) {
            fprintf(stderr, "Authentication by public key failed.\n");
            goto shutdown;
        }
    }
#if 0
    libssh2_trace(session, LIBSSH2_TRACE_CONN);
#endif
    fprintf(stderr, "libssh2_sftp_init().\n");
    do {
        sftp_session = libssh2_sftp_init(session);

        if(!sftp_session) {
            if(libssh2_session_last_errno(session) == LIBSSH2_ERROR_EAGAIN) {
                fprintf(stderr, "non-blocking init\n");
                waitsocket(sock, session); /* now we wait */
            }
            else {
                fprintf(stderr, "Unable to init SFTP session\n");
                goto shutdown;
            }
        }
    } while(!sftp_session);

    fprintf(stderr, "libssh2_sftp_open().\n");
    /* Request a file via SFTP */
    do {
        sftp_handle = libssh2_sftp_open(sftp_session, sftppath,
                                        LIBSSH2_FXF_READ, 0);
        if(!sftp_handle) {
            if(libssh2_session_last_errno(session) != LIBSSH2_ERROR_EAGAIN) {
                fprintf(stderr, "Unable to open file with SFTP: %ld\n",
                        libssh2_sftp_last_error(sftp_session));
                goto shutdown;
            }
            else {
                fprintf(stderr, "non-blocking open\n");
                waitsocket(sock, session); /* now we wait */
            }
        }
    } while(!sftp_handle);

    fprintf(stderr, "libssh2_sftp_open() is done, now receive data.\n");
    do {
        char mem[1024 * 24];
        ssize_t nread;

        /* loop until we fail */
        while((nread = libssh2_sftp_read(sftp_handle, mem, sizeof(mem))) ==
              LIBSSH2_ERROR_EAGAIN) {
            spin++;
            waitsocket(sock, session); /* now we wait */
        }
        if(nread > 0) {
            total += nread;
            write(1, mem, (size_t)nread);
        }
        else {
            break;
        }
    } while(1);

#ifdef HAVE_GETTIMEOFDAY
    gettimeofday(&end, NULL);
    time_ms = tvdiff(end, start);
    fprintf(stderr, "Got %ld bytes in %ld ms = %.1f bytes/sec spin: %d\n",
            (long)total, time_ms,
            (double)total / ((double)time_ms / 1000.0), spin);
#else
    fprintf(stderr, "Got %ld bytes spin: %d\n", (long)total, spin);
#endif

    libssh2_sftp_close(sftp_handle);
    libssh2_sftp_shutdown(sftp_session);

shutdown:

    if(session) {
        fprintf(stderr, "libssh2_session_disconnect\n");
        while(libssh2_session_disconnect(session, "Normal Shutdown") ==
              LIBSSH2_ERROR_EAGAIN);
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
