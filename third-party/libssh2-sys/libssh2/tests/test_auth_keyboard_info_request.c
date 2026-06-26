/* Copyright (C) Xaver Loppenstedt
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

#include "libssh2_priv.h"
#include "userauth_kbd_packet.h"

#include <stdlib.h>

#define PASS 0
#define FAIL -1

struct expected {
    int rc;
    int last_error_code;
    const char *last_error_message;
};
struct test_case {
    const char *data;
    unsigned int data_len;
    struct expected expected;
};

#define TEST_CASES_LEN 16
static const struct test_case
    test_cases[TEST_CASES_LEN] = {
    /* too small */
    {
        NULL, 0,
        {FAIL, -38,
            "userauth keyboard data buffer too small to get length"}},
    /* too small */
    {
        "1234", 4,
        {FAIL, -38,
            "userauth keyboard data buffer too small to get length"}},
    /* smallest valid packet possible */
    {
        "<"
        "\0\0\0\0"
        "\0\0\0\0"
        "\0\0\0\0"
        "\0\0\0\0", 17,
        {PASS, 0, ""}},
    /* overrun name */
    {
        "<"
        "\0\0\0\x7f"
        "\0\0\0\0"
        "\0\0\0\0"
        "\0\0\0\0", 17,
        {FAIL, -6,
            "Unable to decode keyboard-interactive 'name' request field"}},
    /* overrun instruction */
    {
        "<"
        "\0\0\0\0"
        "\0\0\0\x7f"
        "\0\0\0\0"
        "\0\0\0\0", 17,
        {FAIL, -6,
            "Unable to decode keyboard-interactive 'instruction' "
            "request field"}},
    /* overrun language */
    {
        "<"
        "\0\0\0\0"
        "\0\0\0\0"
        "\0\0\0\x7f"
        "\0\0\0\0", 17,
        {FAIL, -6,
            "Unable to decode keyboard-interactive 'language tag' "
            "request field"}},
    /* underrun prompt number */
    {
        "<"
        "\0\0\0\x01"
        "\0\0\0\0"
        "\0\0\0\0"
        "\0\0\0\0", 17,
        {FAIL, -38,
            "Unable to decode keyboard-interactive number of "
            "keyboard prompts"}},
    /* too many prompts */
    {
        "<"
        "\0\0\0\0"
        "\0\0\0\0"
        "\0\0\0\0"
        "\0\0\0\x7f", 17,
        {FAIL, -41, "Too many replies for keyboard-interactive prompts"}},
    /* empty prompt */
    {
        "<"
        "\0\0\0\0"
        "\0\0\0\0"
        "\0\0\0\0"
        "\0\0\0\x01"
        "\0\0\0\0"
        "\0", 22,
        {PASS, 0, ""}},
    /* copied from OpenSSH */
    {
        "<"
        "\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01"
        "\0\0\0\x0aPassword: \0", 32,
        {PASS, 0, ""}},
    /* overrun in prompt text */
    {
        "<"
        "\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01"
        "\0\0\0\x7bPassword: \0", 32,
        {FAIL, -6,
            "Unable to decode keyboard-interactive "
            "prompt message"}},
    /* no echo prompt boolean */
    {
        "<"
        "\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01"
        "\0\0\0\x0bPassword: \0", 32,
        {FAIL, -38, "Unable to decode user auth keyboard prompt echo"}},
    /* two prompts */
    {
        "<"
        "\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x02"
        "\0\0\0\x0aPassword: \0"
        "\0\0\0\x07Token: \1", 44,
        {PASS, 0, ""}},
    /* example from RFC 4256 */
    {
        "<"
        "\0\0\0\x19""CRYPTOCard Authentication"
        "\0\0\0\x1b""The challenge is '14315716'"
        "\0\0\0\x05""en-US"
        "\0\0\0\x01"
        "\0\0\0\x0aResponse: "
        "\x01"
        , 89,
        {PASS, 0, ""}},
    /* three prompts, 3rd missing */
    {
        "<"
        "\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x03"
        "\0\0\0\x0aPassword: \0"
        "\0\0\0\x07Token: \1", 44,
        {FAIL, -6, "Unable to decode keyboard-interactive prompt message"}},
    /* overflow language on 32 bit platform */
    {
        "<"
        "\0\0\0\x19"
            "\0\0\0\x01"
            "\0\0\0\x05""PWN3D\0\1\2\3\4\5\6\7\1\2\3"
            "\x01"
        "\0\0\0\x1b""The challenge is '14315716'"
        "\xff\xff\xff\xc4""en-US"
        "\0\0\0\x01"
        "\0\0\0\x0aResponse: "
        "\x01",
        89,
        {FAIL, -6,
            "Unable to decode keyboard-interactive 'language tag' "
            "request field"}},
};

#define FAILED_MALLOC_TEST_CASES_LEN 2
static const struct test_case
    failed_malloc_test_cases[FAILED_MALLOC_TEST_CASES_LEN] = {
    /* malloc fail */
    {
        "<"
        "\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01"
        "\0\0\0\x0aPassword: \0", 32,
        {FAIL, -6,
            "Unable to allocate memory for "
            "keyboard-interactive prompts array"}},
    /* malloc fail */
    {
        "<"
        "\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01"
        "\0\0\0\x0aPassword: \0", 32,
        {FAIL, -6,
            "Unable to allocate memory for "
            "keyboard-interactive responses array"
        }}
};

static int alloc_count = 0;
static int free_count = 0;

/* libssh2_default_alloc
 */
static
LIBSSH2_ALLOC_FUNC(test_alloc)
{
    int *threshold_int_ptr = *abstract;
    alloc_count++;
    if(*abstract && *threshold_int_ptr == alloc_count) {
        return NULL;
    }

    return malloc(count);
}

/* libssh2_default_free
 */
static
LIBSSH2_FREE_FUNC(test_free)
{
    (void)abstract;
    free_count++;
    free(ptr);
}

static
int test_case(int num,
              const char *data, unsigned int data_len, void *abstract,
              struct expected expected)
{
    int rc;
    char *message;
    int error_code;
    LIBSSH2_SESSION *session;

    alloc_count = 0;
    free_count = 0;

    session = libssh2_session_init_ex(test_alloc, test_free, NULL, abstract);
    if(!session) {
        fprintf(stderr, "libssh2_session_init_ex failed\n");
        return 1;
    }

    session->userauth_kybd_data = LIBSSH2_ALLOC(session, data_len);
    session->userauth_kybd_data_len = data_len;
    memcpy(session->userauth_kybd_data, data, data_len);

    rc = userauth_keyboard_interactive_decode_info_request(session);

    if(rc != expected.rc) {
        fprintf(stdout,
                "Test case %d: expected return code to be %d got %d\n",
                num, expected.rc, rc);
        return 1;
    }

    error_code = libssh2_session_last_error(session, &message, NULL, 0);

    if(expected.last_error_code != error_code) {
        fprintf(stdout,
                "Test case %d: expected last error code to be "
                "\"%d\" got \"%d\"\n",
                num, expected.last_error_code, error_code);
        return 1;
    }

    if(strcmp(expected.last_error_message, message) != 0) {
        fprintf(stdout,
                "Test case %d: expected last error message to be "
                "\"%s\" got \"%s\"\n",
                num, expected.last_error_message, message);
        return 1;
    }
    libssh2_session_free(session);

    fprintf(stderr, "Test case %d passed\n", num);

    return 0;
}

int main(void)
{
    int ret = 0;
    int i;

    for(i = 0; i < TEST_CASES_LEN; i++) {
        if(test_case(i + 1,
                     test_cases[i].data,
                     test_cases[i].data_len,
                     NULL,
                     test_cases[i].expected))
            ret = 1;
    }

    for(i = 0; i < FAILED_MALLOC_TEST_CASES_LEN; i++) {
        int tc =  i + TEST_CASES_LEN + 1;
        int malloc_call_num = 3 + i;
        if(test_case(tc,
                     failed_malloc_test_cases[i].data,
                     failed_malloc_test_cases[i].data_len,
                     &malloc_call_num,
                     failed_malloc_test_cases[i].expected))
            ret = 1;
    }

    return ret;
}
