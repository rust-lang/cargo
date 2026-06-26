/* Copyright (C) The libssh2 project and its contributors.
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
#include <string.h>

#ifndef INADDR_NONE
#define INADDR_NONE (in_addr_t)~0
#endif

static const char *pubkey = "/home/username/.ssh/id_rsa.pub";
static const char *privkey = "/home/username/.ssh/id_rsa";
static const char *username = "username";
static const char *password = "";

static const char *server_ip = "127.0.0.1";

enum {
    AUTH_NONE = 0,
    AUTH_PASSWORD = 1,
    AUTH_PUBLICKEY = 2
};

static int netconf_write(LIBSSH2_CHANNEL *channel, const char *buf, size_t len)
{
    ssize_t i;
    ssize_t wr = 0;

    do {
        i = libssh2_channel_write(channel, buf, len);
        if(i < 0) {
            fprintf(stderr, "libssh2_channel_write: %ld\n", (long)i);
            return -1;
        }
        wr += i;
    } while(i > 0 && wr < (ssize_t)len);

    return 0;
}

static ssize_t netconf_read_until(LIBSSH2_CHANNEL *channel, const char *endtag,
                                  char *buf, size_t buflen)
{
    ssize_t len;
    size_t rd = 0;
    char *endreply = NULL, *specialsequence = NULL;

    memset(buf, 0, buflen);

    do {
        len = libssh2_channel_read(channel, buf + rd, buflen - rd);
        if(LIBSSH2_ERROR_EAGAIN == len)
            continue;
        else if(len < 0) {
            fprintf(stderr, "libssh2_channel_read: %ld\n", (long)len);
            return -1;
        }
        rd += (size_t)len;

        /* read more data until we see a rpc-reply closing tag followed by
         * the special sequence ]]>]]> */

        /* really, this MUST be replaced with proper XML parsing. */

        endreply = strstr(buf, endtag);
        if(endreply)
            specialsequence = strstr(endreply, "]]>]]>");

    } while(!specialsequence && rd < buflen);

    if(!specialsequence) {
        fprintf(stderr, "netconf_read_until(): ]]>]]> not found."
                        " Read buffer too small?\n");
        return -1;
    }

    /* discard the special sequence so that only XML is returned */
    rd = (size_t)(specialsequence - buf);
    buf[rd] = 0;

    return (ssize_t)rd;
}

int main(int argc, char *argv[])
{
    int i, auth = AUTH_NONE;
    struct sockaddr_in sin;
    const char *fingerprint;
    char *userauthlist;
    int rc;
    LIBSSH2_SESSION *session = NULL;
    LIBSSH2_CHANNEL *channel = NULL;
    char buf[1048576]; /* avoid any buffer reallocation for simplicity */
    ssize_t len;
    libssh2_socket_t sock;

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
    sin.sin_port = htons(830);
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
        if(argc > 4) {
            if((auth & AUTH_PASSWORD) && !strcmp(argv[4], "-p"))
                auth = AUTH_PASSWORD;
            if((auth & AUTH_PUBLICKEY) && !strcmp(argv[4], "-k"))
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

    /* open a channel */
    channel = libssh2_channel_open_session(session);
    if(!channel) {
        fprintf(stderr, "Could not open the channel.\n"
                        "(Note that this can be a problem at the server."
                        " Please review the server logs.)\n");
        goto shutdown;
    }

    /* execute the subsystem on our channel */
    if(libssh2_channel_subsystem(channel, "netconf")) {
        fprintf(stderr, "Could not execute the 'netconf' subsystem.\n"
                        "(Note that this can be a problem at the server."
                        " Please review the server logs.)\n");
        goto shutdown;
    }

    /* NETCONF: https://tools.ietf.org/html/draft-ietf-netconf-ssh-06 */

    fprintf(stderr, "Sending NETCONF client <hello>\n");
    len = snprintf(buf, sizeof(buf),
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
        "<hello>"
        "<capabilities>"
        "<capability>urn:ietf:params:xml:ns:netconf:base:1.0</capability>"
        "</capabilities>"
        "</hello>\n"
        "]]>]]>\n");
    if(len < 0)
        goto shutdown;
    if(-1 == netconf_write(channel, buf, (size_t)len))
        goto shutdown;

    fprintf(stderr, "Reading NETCONF server <hello>\n");
    len = netconf_read_until(channel, "</hello>", buf, sizeof(buf));
    if(-1 == len)
        goto shutdown;

    fprintf(stderr, "Got %ld bytes:\n----------------------\n%s",
            (long)len, buf);

    fprintf(stderr, "Sending NETCONF <rpc>\n");
    len = snprintf(buf, sizeof(buf),
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
        "<rpc xmlns=\"urn:ietf:params:xml:ns:netconf:base:1.0\">"
        "<get-interface-information><terse/></get-interface-information>"
        "</rpc>\n"
        "]]>]]>\n");
    if(len < 0)
        goto shutdown;
    if(-1 == netconf_write(channel, buf, (size_t)len))
        goto shutdown;

    fprintf(stderr, "Reading NETCONF <rpc-reply>\n");
    len = netconf_read_until(channel, "</rpc-reply>", buf, sizeof(buf));
    if(-1 == len)
        goto shutdown;

    fprintf(stderr, "Got %ld bytes:\n----------------------\n%s",
            (long)len, buf);

shutdown:

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
