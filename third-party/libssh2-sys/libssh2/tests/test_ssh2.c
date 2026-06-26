/* Copyright (C) The libssh2 project and its contributors.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 *
 * Self test, based on example/ssh2.c.
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
#include <stdlib.h>  /* for getenv() */

static const char *hostname = "127.0.0.1";
static const int port_number = 4711;
static const char *pubkey = "key_rsa.pub";
static const char *privkey = "key_rsa";
static const char *username = "username";
static const char *password = "password";

static void portable_sleep(unsigned int seconds)
{
#ifdef _WIN32
    Sleep(seconds);
#else
    sleep(seconds);
#endif
}

int main(int argc, char *argv[])
{
    uint32_t hostaddr;
    libssh2_socket_t sock;
    int i, auth_pw = 0;
    struct sockaddr_in sin;
    const char *fingerprint;
    char *userauthlist;
    int rc;
    LIBSSH2_SESSION *session = NULL;
    LIBSSH2_CHANNEL *channel;
    unsigned int counter;

#ifdef _WIN32
    WSADATA wsadata;

    rc = WSAStartup(MAKEWORD(2, 0), &wsadata);
    if(rc) {
        fprintf(stderr, "WSAStartup failed with error: %d\n", rc);
        return 1;
    }
#endif

    (void)argc;
    (void)argv;

    #ifdef _WIN32
    #define LIBSSH2_FALLBACK_USER_ENV "USERNAME"
    #else
    #define LIBSSH2_FALLBACK_USER_ENV "LOGNAME"
    #endif

    if(getenv("USER"))
        username = getenv("USER");
    else if(getenv(LIBSSH2_FALLBACK_USER_ENV))
        username = getenv(LIBSSH2_FALLBACK_USER_ENV);

    if(getenv("PRIVKEY"))
        privkey = getenv("PRIVKEY");

    if(getenv("PUBKEY"))
        pubkey = getenv("PUBKEY");

    hostaddr = inet_addr(hostname);
    if(hostaddr == (uint32_t)(-1)) {
        fprintf(stderr, "Failed to convert %s host address\n", hostname);
        return 1;
    }

    rc = libssh2_init(0);
    if(rc) {
        fprintf(stderr, "libssh2 initialization failed (%d)\n", rc);
        return 1;
    }

    rc = 1;

    sock = socket(AF_INET, SOCK_STREAM, 0);
    if(sock == LIBSSH2_INVALID_SOCKET) {
        fprintf(stderr, "failed to create socket.\n");
        goto shutdown;
    }

    sin.sin_family = AF_INET;
    sin.sin_port = htons((unsigned short)port_number);
    sin.sin_addr.s_addr = hostaddr;

    for(counter = 0; counter < 3; ++counter) {
        if(connect(sock, (struct sockaddr*)(&sin),
                   sizeof(struct sockaddr_in))) {
            fprintf(stderr,
                    "Connection to %s:%d attempt #%d failed: retrying...\n",
                    hostname, port_number, counter);
            portable_sleep(1 + 2*counter);
        }
        else {
            break;
        }
    }
    if(sock == LIBSSH2_INVALID_SOCKET) {
        fprintf(stderr, "Failed to connect to %s:%d\n",
                hostname, port_number);
        goto shutdown;
    }

    /* Create a session instance and start it up. This will trade welcome
     * banners, exchange keys, and setup crypto, compression, and MAC layers
     */
    session = libssh2_session_init();
    if(!session) {
        fprintf(stderr, "Could not initialize SSH session.\n");
        goto shutdown;
    }

    if(getenv("FIXTURE_TRACE_ALL_CONNECT") ||
       getenv("FIXTURE_TRACE_ALL")) {
        libssh2_trace(session, ~0);
        fprintf(stdout, "Trace all enabled.\n");
    }

    libssh2_session_set_blocking(session, 1);

    {
        int retries = 0, retry = 0;
#ifdef LIBSSH2_WINCNG
        /* FIXME: Retry tests with WinCNG due to flakiness in hostkey
           verification: https://github.com/libssh2/libssh2/issues/804 */
        retries += 2;
#endif
        do {
            rc = libssh2_session_handshake(session, sock);
            if(rc == 0) {
                break;
            }
            fprintf(stderr, "Failure establishing SSH session: %d\n", rc);
            if(
#ifdef LIBSSH2_WINCNG
               rc != LIBSSH2_ERROR_KEY_EXCHANGE_FAILURE ||
#endif
               ++retry > retries) {
                break;
            }
            fprintf(stderr, "Retrying... %d / %d\n", retry, retries);
        } while(1);
    }

    rc = 1;

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

    /* check what authentication methods are available */
    userauthlist = libssh2_userauth_list(session, username,
                                         (unsigned int)strlen(username));
    if(userauthlist) {
        fprintf(stderr, "Authentication methods: %s\n", userauthlist);
        if(strstr(userauthlist, "password")) {
            auth_pw |= 1;
        }
        if(strstr(userauthlist, "keyboard-interactive")) {
            auth_pw |= 2;
        }
        if(strstr(userauthlist, "publickey")) {
            auth_pw |= 4;
        }

        if(auth_pw & 4) {
            /* Authenticate by public key */
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

    /* Request a session channel on which to run a shell */
    channel = libssh2_channel_open_session(session);
    if(!channel) {
        fprintf(stderr, "Unable to open a session\n");
        goto shutdown;
    }

    /* Some environment variables may be set,
     * It's up to the server which ones it'll allow though
     */
    libssh2_channel_setenv(channel, "FOO", "bar");

    /* Request a terminal with 'vanilla' terminal emulation
     * See /etc/termcap for more options. This is useful when opening
     * an interactive shell.
     */
    if(libssh2_channel_request_pty(channel, "vanilla")) {
        fprintf(stderr, "Failed requesting pty\n");
        goto skip_shell;
    }

    /* Open a SHELL on that pty */
    if(libssh2_channel_shell(channel)) {
        fprintf(stderr, "Unable to request shell on allocated pty\n");
        goto shutdown;
    }

    rc = 0;

skip_shell:

    if(channel) {
        libssh2_channel_free(channel);
        channel = NULL;
    }

shutdown:

    if(session) {
        libssh2_session_disconnect(session, "Normal Shutdown");
        libssh2_session_free(session);
    }

    if(sock != LIBSSH2_INVALID_SOCKET) {
        shutdown(sock, 2 /* SHUT_RDWR */);
#ifdef _WIN32
        closesocket(sock);
#else
        close(sock);
#endif
    }

    fprintf(stderr, "all done\n");

    libssh2_exit();

#ifdef _WIN32
    WSACleanup();
#endif

    return rc;
}
