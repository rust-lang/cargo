/* Copyright (C) The libssh2 project and its contributors.
 *
 * Sample showing how to do SFTP transfers.
 *
 * The sample code has default values for host name, user name, password
 * and path to copy, but you can specify them on the command line like:
 *
 * $ ./sftp 192.168.0.1 user password /tmp/secrets -p|-i|-k
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

#include <stdio.h>
#include <string.h>

static const char *pubkey = "/home/username/.ssh/id_rsa.pub";
static const char *privkey = "/home/username/.ssh/id_rsa";
static const char *username = "username";
static const char *password = "password";
static const char *sftppath = "/tmp/TEST";

static void kbd_callback(const char *name, int name_len,
                         const char *instruction, int instruction_len,
                         int num_prompts,
                         const LIBSSH2_USERAUTH_KBDINT_PROMPT *prompts,
                         LIBSSH2_USERAUTH_KBDINT_RESPONSE *responses,
                         void **abstract)
{
    int i;
    size_t n;
    char buf[1024];
    (void)abstract;

    fprintf(stderr, "Performing keyboard-interactive authentication.\n");

    fprintf(stderr, "Authentication name: '");
    fwrite(name, 1, (size_t)name_len, stderr);
    fprintf(stderr, "'\n");

    fprintf(stderr, "Authentication instruction: '");
    fwrite(instruction, 1, (size_t)instruction_len, stderr);
    fprintf(stderr, "'\n");

    fprintf(stderr, "Number of prompts: %d\n\n", num_prompts);

    for(i = 0; i < num_prompts; i++) {
        fprintf(stderr, "Prompt %d from server: '", i);
        fwrite(prompts[i].text, 1, prompts[i].length, stderr);
        fprintf(stderr, "'\n");

        fprintf(stderr, "Please type response: ");
        fgets(buf, sizeof(buf), stdin);
        n = strlen(buf);
        while(n > 0 && strchr("\r\n", buf[n - 1]))
            n--;
        buf[n] = 0;

        responses[i].text = strdup(buf);
        responses[i].length = (unsigned int)n;

        fprintf(stderr, "Response %d from user is '", i);
        fwrite(responses[i].text, 1, responses[i].length, stderr);
        fprintf(stderr, "'\n\n");
    }

    fprintf(stderr,
        "Done. Sending keyboard-interactive responses to server now.\n");
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
    LIBSSH2_SFTP *sftp_session;
    LIBSSH2_SFTP_HANDLE *sftp_handle;

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

    /* Since we have set non-blocking, tell libssh2 we are blocking */
    libssh2_session_set_blocking(session, 1);

    /* ... start it up. This will trade welcome banners, exchange keys,
     * and setup crypto, compression, and MAC layers
     */
    rc = libssh2_session_handshake(session, sock);
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

        /* check for options */
        if(argc > 5) {
            if((auth_pw & 1) && !strcmp(argv[5], "-p")) {
                auth_pw = 1;
            }
            if((auth_pw & 2) && !strcmp(argv[5], "-i")) {
                auth_pw = 2;
            }
            if((auth_pw & 4) && !strcmp(argv[5], "-k")) {
                auth_pw = 4;
            }
        }

        if(auth_pw & 1) {
            /* We could authenticate via password */
            if(libssh2_userauth_password(session, username, password)) {
                fprintf(stderr, "Authentication by password failed.\n");
                goto shutdown;
            }
        }
        else if(auth_pw & 2) {
            /* Or via keyboard-interactive */
            if(libssh2_userauth_keyboard_interactive(session, username,
                                                     &kbd_callback) ) {
                fprintf(stderr,
                        "Authentication by keyboard-interactive failed.\n");
                goto shutdown;
            }
            else {
                fprintf(stderr,
                        "Authentication by keyboard-interactive succeeded.\n");
            }
        }
        else if(auth_pw & 4) {
            /* Or by public key */
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

    fprintf(stderr, "libssh2_sftp_init().\n");
    sftp_session = libssh2_sftp_init(session);

    if(!sftp_session) {
        fprintf(stderr, "Unable to init SFTP session\n");
        goto shutdown;
    }

    fprintf(stderr, "libssh2_sftp_open().\n");
    /* Request a file via SFTP */
    sftp_handle = libssh2_sftp_open(sftp_session, sftppath,
                                    LIBSSH2_FXF_READ, 0);
    if(!sftp_handle) {
        fprintf(stderr, "Unable to open file with SFTP: %ld\n",
                libssh2_sftp_last_error(sftp_session));
        goto shutdown;
    }

    fprintf(stderr, "libssh2_sftp_open() is done, now receive data.\n");
    do {
        char mem[1024];
        ssize_t nread;

        /* loop until we fail */
        fprintf(stderr, "libssh2_sftp_read().\n");
        nread = libssh2_sftp_read(sftp_handle, mem, sizeof(mem));
        if(nread > 0) {
            write(1, mem, (size_t)nread);
        }
        else {
            break;
        }
    } while(1);

    libssh2_sftp_close(sftp_handle);
    libssh2_sftp_shutdown(sftp_session);

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
