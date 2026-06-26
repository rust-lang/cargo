/* Copyright (C) Xaver Loppenstedt <xaver@loppenstedt.de>
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

int userauth_keyboard_interactive_decode_info_request(LIBSSH2_SESSION *session)
{
    unsigned char *language_tag;
    size_t language_tag_len;
    unsigned int i;
    unsigned char packet_type;
    uint32_t tmp_u32;

    struct string_buf decoded;

    decoded.data = session->userauth_kybd_data;
    decoded.dataptr = session->userauth_kybd_data;
    decoded.len = session->userauth_kybd_data_len;

    if(session->userauth_kybd_data_len < 17) {
        _libssh2_error(session, LIBSSH2_ERROR_BUFFER_TOO_SMALL,
                       "userauth keyboard data buffer too small "
                       "to get length");
        return -1;
    }

    /* byte      SSH_MSG_USERAUTH_INFO_REQUEST */
    _libssh2_get_byte(&decoded, &packet_type);

    /* string    name (ISO-10646 UTF-8) */
    if(_libssh2_copy_string(session, &decoded,
                            &session->userauth_kybd_auth_name,
                            &session->userauth_kybd_auth_name_len) == -1) {
        _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                       "Unable to decode "
                       "keyboard-interactive 'name' "
                       "request field");
        return -1;
    }

    /* string    instruction (ISO-10646 UTF-8) */
    if(_libssh2_copy_string(session, &decoded,
                            &session->userauth_kybd_auth_instruction,
                            &session->userauth_kybd_auth_instruction_len)
       == -1) {
        _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                       "Unable to decode "
                       "keyboard-interactive 'instruction' "
                       "request field");
        return -1;
    }

    /* string    language tag (as defined in [RFC-3066]) */
    if(_libssh2_get_string(&decoded, &language_tag,
                           &language_tag_len) == -1) {
        _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                       "Unable to decode "
                       "keyboard-interactive 'language tag' "
                       "request field");
        return -1;
    }

    /* int       num-prompts */
    if(_libssh2_get_u32(&decoded, &tmp_u32) == -1 ||
       (session->userauth_kybd_num_prompts = tmp_u32) != tmp_u32) {
        _libssh2_error(session, LIBSSH2_ERROR_BUFFER_TOO_SMALL,
                       "Unable to decode "
                       "keyboard-interactive number of keyboard prompts");
        return -1;
    }

    if(session->userauth_kybd_num_prompts > 100) {
        _libssh2_error(session, LIBSSH2_ERROR_OUT_OF_BOUNDARY,
                       "Too many replies for "
                       "keyboard-interactive prompts");
        return -1;
    }

    if(session->userauth_kybd_num_prompts == 0) {
        return 0;
    }

    session->userauth_kybd_prompts =
        LIBSSH2_CALLOC(session,
                       sizeof(LIBSSH2_USERAUTH_KBDINT_PROMPT) *
                       session->userauth_kybd_num_prompts);
    if(!session->userauth_kybd_prompts) {
        _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                       "Unable to allocate memory for "
                       "keyboard-interactive prompts array");
        return -1;
    }

    session->userauth_kybd_responses =
        LIBSSH2_CALLOC(session,
                       sizeof(LIBSSH2_USERAUTH_KBDINT_RESPONSE) *
                       session->userauth_kybd_num_prompts);
    if(!session->userauth_kybd_responses) {
        _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                       "Unable to allocate memory for "
                       "keyboard-interactive responses array");
        return -1;
    }

    for(i = 0; i < session->userauth_kybd_num_prompts; i++) {
        /* string    prompt[1] (ISO-10646 UTF-8) */
        if(_libssh2_copy_string(session, &decoded,
                                &session->userauth_kybd_prompts[i].text,
                                &session->userauth_kybd_prompts[i].length)
           == -1) {
            _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                           "Unable to decode "
                           "keyboard-interactive prompt message");
            return -1;
        }

        /* boolean   echo[1] */
        if(_libssh2_get_boolean(&decoded,
                                &session->userauth_kybd_prompts[i].echo)
           == -1) {
            _libssh2_error(session, LIBSSH2_ERROR_BUFFER_TOO_SMALL,
                           "Unable to decode "
                           "user auth keyboard prompt echo");
            return -1;
        }
    }

    return 0;
}
