/* Copyright (C) Alexander Lamaison
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms,
 * with or without modification, are permitted provided
 * that the following conditions are met:
 *
 *   Redistributions of source code must retain the above
 *   copyright notice, this list of conditions and the
 *   following disclaimer.
 *
 *   Redistributions in binary form must reproduce the above
 *   copyright notice, this list of conditions and the following
 *   disclaimer in the documentation and/or other materials
 *   provided with the distribution.
 *
 *   Neither the name of the copyright holder nor the names
 *   of any other contributors may be used to endorse or
 *   promote products derived from this software without
 *   specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND
 * CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES,
 * INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES
 * OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR
 * CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
 * SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
 * BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
 * INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
 * WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
 * NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
 * USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY
 * OF SUCH DAMAGE.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

#include "session_fixture.h"
#include "openssh_fixture.h"

#ifdef HAVE_SYS_SOCKET_H
#include <sys/socket.h>
#endif
#ifdef HAVE_UNISTD_H
#include <unistd.h>
#endif

#include <stdio.h>
#include <stdlib.h>  /* for getenv() */
#include <assert.h>

static LIBSSH2_SESSION *connected_session = NULL;
static libssh2_socket_t connected_socket = LIBSSH2_INVALID_SOCKET;

static int connect_to_server(void)
{
    int rc;
    connected_socket = open_socket_to_openssh_server();
    if(connected_socket == LIBSSH2_INVALID_SOCKET) {
        return LIBSSH2_ERROR_SOCKET_NONE;
    }

    rc = libssh2_session_handshake(connected_session, connected_socket);
    if(rc) {
        print_last_session_error("libssh2_session_handshake");
        return libssh2_session_last_errno(connected_session);
    }

    return LIBSSH2_ERROR_NONE;
}

/* List of crypto protocols for which tests are skipped */
static char const *skip_crypt[] = {
#ifdef LIBSSH2_MBEDTLS
    /* Due to a bug with mbedTLS support, these crypt methods fail.
       Until that bug is fixed, don't run them there to avoid this
       known issue causing red tests.
       See: https://github.com/libssh2/libssh2/issues/793
     */
    "3des-cbc",
    "aes128-cbc",
    "aes192-cbc",
    "aes256-cbc",
    "aes128-gcm@openssh.com",
    "aes256-gcm@openssh.com",
    "rijndael-cbc@lysator.liu.se",
#endif

#if !LIBSSH2_3DES
    "3des-cbc",
#endif

#if !LIBSSH2_AES_GCM
    /* Support for AES-GCM hasn't been added to these back-ends yet */
    "aes128-gcm@openssh.com",
    "aes256-gcm@openssh.com",
#endif

    NULL
};

/* List of MAC protocols for which tests are skipped */
static char const *skip_mac[] = {
#if !LIBSSH2_MD5
    "hmac-md5",
    "hmac-md5-96",
#endif
    NULL
};

LIBSSH2_SESSION *start_session_fixture(int *skipped, int *err)
{
    int rc;

    const char *crypt = getenv("FIXTURE_TEST_CRYPT");
    const char *mac = getenv("FIXTURE_TEST_MAC");

    *skipped = 0;
    *err = LIBSSH2_ERROR_NONE;

    if(crypt) {
        char const * const *sk;
        for(sk = skip_crypt; *sk; ++sk) {
            if(strcmp(*sk, crypt) == 0) {
                fprintf(stderr, "unsupported crypt algorithm (%s) skipped.\n",
                                crypt);
                *skipped = 1;
                return NULL;
            }
        }
    }

    if(mac) {
        char const * const *sk;
        for(sk = skip_mac; *sk; ++sk) {
            if(strcmp(*sk, mac) == 0) {
                fprintf(stderr, "unsupported MAC algorithm (%s) skipped.\n",
                                mac);
                *skipped = 1;
                return NULL;
            }
        }
    }

    rc = start_openssh_fixture();
    if(rc) {
        return NULL;
    }
    rc = libssh2_init(0);
    if(rc) {
        fprintf(stderr, "libssh2_init failed (%d)\n", rc);
        return NULL;
    }

    connected_session = libssh2_session_init_ex(NULL, NULL, NULL, NULL);
    if(!connected_session) {
        fprintf(stderr, "libssh2_session_init_ex failed\n");
        return NULL;
    }

    if(getenv("FIXTURE_TRACE_ALL_CONNECT")) {
        libssh2_trace(connected_session, ~0);
        fprintf(stdout, "Trace all enabled for connect_to_server.\n");
    }
    else if(getenv("FIXTURE_TRACE_ALL")) {
        libssh2_trace(connected_session, ~0);
        fprintf(stdout, "Trace all enabled.\n");
    }

    /* Override crypt algorithm for the test */
    if(crypt) {
        if(libssh2_session_method_pref(connected_session,
                                       LIBSSH2_METHOD_CRYPT_CS, crypt) ||
           libssh2_session_method_pref(connected_session,
                                       LIBSSH2_METHOD_CRYPT_SC, crypt)) {
            fprintf(stderr, "libssh2_session_method_pref CRYPT failed "
                            "(probably disabled in the build): '%s'\n", crypt);
            return NULL;
        }
    }

    /* Override mac algorithm for the test */
    if(mac) {
        if(libssh2_session_method_pref(connected_session,
                                       LIBSSH2_METHOD_MAC_CS, mac) ||
           libssh2_session_method_pref(connected_session,
                                       LIBSSH2_METHOD_MAC_SC, mac)) {
            fprintf(stderr, "libssh2_session_method_pref MAC failed "
                            "(probably disabled in the build): '%s'\n", mac);
            return NULL;
        }
    }

    libssh2_session_set_blocking(connected_session, 1);

    rc = connect_to_server();
    if(rc != LIBSSH2_ERROR_NONE) {
        *err = rc;
        return NULL;
    }

    if(getenv("FIXTURE_TRACE_ALL_CONNECT")) {
        libssh2_trace(connected_session, 0);
    }

    return connected_session;
}

void print_last_session_error(const char *function)
{
    if(connected_session) {
        char *message;
        int rc =
            libssh2_session_last_error(connected_session, &message, NULL, 0);
        fprintf(stderr, "%s failed (%d): %s\n", function, rc, message);
    }
    else {
        fprintf(stderr, "No session\n");
    }
}

void stop_session_fixture(void)
{
    if(connected_session) {
        libssh2_session_disconnect(connected_session, "test ended");
        libssh2_session_free(connected_session);
        connected_session = NULL;
    }
    else {
        fprintf(stderr, "Cannot stop session - none started\n");
    }

    close_socket_to_openssh_server(connected_socket);
    connected_socket = LIBSSH2_INVALID_SOCKET;

    srcdir_path(NULL);  /* cleanup allocated filepath */

    libssh2_exit();

    stop_openssh_fixture();
}


/* Return a static string that contains a file path relative to the srcdir
 * variable, if found.
 */
#define NUMPATHS 32
char *srcdir_path(const char *file)
{
    static char *filepath[NUMPATHS];
    static int curpath;
    char *p = getenv("srcdir");
    if(file) {
        int len;
        if(curpath >= NUMPATHS) {
            fprintf(stderr, "srcdir_path ran out of filepath slots.\n");
        }
        assert(curpath < NUMPATHS);
        if(p) {
            len = snprintf(NULL, 0, "%s/%s", p, file);
            if(len > 2) {
                filepath[curpath] = calloc(1, (size_t)len + 1);
                snprintf(filepath[curpath], (size_t)len + 1, "%s/%s", p, file);
            }
            else {
               return NULL;
            }
        }
        else {
            len = snprintf(NULL, 0, "%s", file);
            if(len > 0) {
                filepath[curpath] = calloc(1, (size_t)len + 1);
                snprintf(filepath[curpath], (size_t)len + 1, "%s", file);
            }
            else {
               return NULL;
            }
        }
        return filepath[curpath++];
    }
    else {
        int i;
        for(i = 0; i < curpath; ++i) {
            free(filepath[curpath]);
        }
        curpath = 0;
        return NULL;
    }
}

static const char *kbd_password;

static void kbd_callback(const char *name, int name_len,
                         const char *instruct, int instruct_len,
                         int num_prompts,
                         const LIBSSH2_USERAUTH_KBDINT_PROMPT *prompts,
                         LIBSSH2_USERAUTH_KBDINT_RESPONSE *responses,
                         void **abstract)
{
    int i;
    (void)abstract;

    fprintf(stdout, "Kb-int name: %.*s\n", name_len, name);
    fprintf(stdout, "Kb-int instruction: %.*s\n", instruct_len, instruct);
    for(i = 0; i < num_prompts; ++i) {
        fprintf(stdout, "Kb-int prompt %d: %.*s\n", i,
                (int)prompts[i].length, prompts[i].text);
    }

    if(num_prompts == 1) {
        responses[0].text = strdup(kbd_password);
        responses[0].length = (unsigned int)strlen(kbd_password);
    }
}

int test_auth_keyboard(LIBSSH2_SESSION *session, int flags,
                       const char *username,
                       const char *password)
{
    int rc;

    const char *userauth_list =
        libssh2_userauth_list(session, username,
                              (unsigned int)strlen(username));
    if(!userauth_list) {
        print_last_session_error("libssh2_userauth_list");
        return 1;
    }

    if(!strstr(userauth_list, "keyboard-interactive")) {
        fprintf(stderr,
                "'keyboard-interactive' was expected in userauth list: %s\n",
                userauth_list);
        return 1;
    }

    kbd_password = password;

    rc = libssh2_userauth_keyboard_interactive_ex(session, username,
                                                (unsigned int)strlen(username),
                                                  kbd_callback);

    kbd_password = NULL;

    if((flags & TEST_AUTH_SHOULDFAIL) != 0) {
        if(rc == 0) {
            fprintf(stderr, "Keyboard-interactive auth succeeded "
                            "with wrong response\n");
            return 1;
        }
    }
    else {
        if(rc) {
            print_last_session_error(
                "libssh2_userauth_keyboard_interactive_ex");
            return 1;
        }
    }

    return 0;
}

int test_auth_password(LIBSSH2_SESSION *session, int flags,
                       const char *username,
                       const char *password)
{
    int rc;

    const char *userauth_list =
        libssh2_userauth_list(session, username,
                              (unsigned int)strlen(username));
    if(!userauth_list) {
        print_last_session_error("libssh2_userauth_list");
        return 1;
    }

    if(!strstr(userauth_list, "password")) {
        fprintf(stderr, "'password' was expected in userauth list: %s\n",
                userauth_list);
        return 1;
    }

    rc = libssh2_userauth_password_ex(session, username,
                                      (unsigned int)strlen(username),
                                      password,
                                      (unsigned int)strlen(password),
                                      NULL);

    if((flags & TEST_AUTH_SHOULDFAIL) != 0) {
        if(rc == 0) {
            fprintf(stderr, "Password auth succeeded with wrong password\n");
            return 1;
        }
    }
    else {
        if(rc) {
            print_last_session_error("libssh2_userauth_password_ex");
            return 1;
        }

        if(libssh2_userauth_authenticated(session) == 0) {
            fprintf(stderr, "Password auth appeared to succeed but "
                            "libssh2_userauth_authenticated returned 0\n");
            return 1;
        }
    }

    return 0;
}

static int read_file(const char *path, char **out_buffer, size_t *out_len)
{
    FILE *fp;
    char *buffer;
    ssize_t len;

    if(!out_buffer || !out_len || !path) {
        fprintf(stderr, "invalid params.\n");
        return 1;
    }

    *out_buffer = NULL;
    *out_len = 0;

    fp = fopen(path, "r");

    if(!fp) {
        fprintf(stderr, "File could not be read: %s\n", path);
        return 1;
    }

    fseek(fp, 0L, SEEK_END);
    len = ftell(fp);
    if(len < 0) {
        fclose(fp);
        fprintf(stderr, "Could not determine input size of: %s\n", path);
        return 1;
    }
    rewind(fp);

    buffer = calloc(1, (size_t)len + 1);
    if(!buffer) {
        fclose(fp);
        fprintf(stderr, "Could not alloc memory.\n");
        return 1;
    }

    if(1 != fread(buffer, (size_t)len, 1, fp)) {
        fclose(fp);
        free(buffer);
        fprintf(stderr, "Could not read file into memory.\n");
        return 1;
    }

    fclose(fp);

    *out_buffer = buffer;
    *out_len = (size_t)len;

    return 0;
}

int test_auth_pubkey(LIBSSH2_SESSION *session, int flags,
                     const char *username,
                     const char *password,
                     const char *fn_pub,
                     const char *fn_priv)
{
    int rc;
    const char *userauth_list;

    /* Ignore our hard-wired Dockerfile user when not running under Docker */
    if(!openssh_fixture_have_docker() && strcmp(username, "libssh2") == 0) {
        username = getenv("USER");
        if(!username) {
#ifdef _WIN32
            username = getenv("USERNAME");
#else
            username = getenv("LOGNAME");
#endif
        }
    }

    userauth_list = libssh2_userauth_list(session, username,
                                          (unsigned int)strlen(username));
    if(!userauth_list) {
        print_last_session_error("libssh2_userauth_list");
        return 1;
    }

    if(!strstr(userauth_list, "publickey")) {
        fprintf(stderr, "'publickey' was expected in userauth list: %s\n",
                userauth_list);
        return 1;
    }

    if((flags & TEST_AUTH_FROMMEM) != 0) {
        char *buffer = NULL;
        size_t len = 0;

        if(read_file(srcdir_path(fn_priv), &buffer, &len)) {
            fprintf(stderr, "Reading key file failed.\n");
            return 1;
        }

        rc = libssh2_userauth_publickey_frommemory(session,
                                                   username, strlen(username),
                                                   NULL, 0,
                                                   buffer, len,
                                                   NULL);

        free(buffer);
    }
    else {
        rc = libssh2_userauth_publickey_fromfile_ex(session, username,
                                                (unsigned int)strlen(username),
                                                    srcdir_path(fn_pub),
                                                    srcdir_path(fn_priv),
                                                    password);
    }

    if((flags & TEST_AUTH_SHOULDFAIL) != 0) {
        if(rc == 0) {
            fprintf(stderr, "Public-key auth succeeded with wrong key\n");
            return 1;
        }
    }
    else {
        if(rc) {
            print_last_session_error("libssh2_userauth_publickey_fromfile_ex");
            return 1;
        }
    }

    return 0;
}
