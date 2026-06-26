/*
 * Copyright (C) Daiki Ueno
 * Copyright (C) Daniel Stenberg
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
 * SPDX-License-Identifier: BSD-3-Clause AND BSD-2-Clause
 */

#ifdef HAVE_WIN32_AGENTS /* Compile this via agent.c */

#include <stdlib.h>  /* for getenv() */

/* Code to talk to OpenSSH was taken and modified from the Win32 port of
 * Portable OpenSSH by the PowerShell team. Commit
 * 8ab565c53f3619d6a1f5ac229e212cad8a52852c of
 * https://github.com/PowerShell/openssh-portable.git was used as the base,
 * specifically the following files:
 *
 * - contrib\win32\win32compat\fileio.c
 *   - Structure of agent_connect_openssh from ssh_get_authentication_socket
 *   - Structure of agent_transact_openssh from ssh_request_reply
 * - contrib\win32\win32compat\wmain_common.c
 *   - Windows equivalent functions for common Unix functions, inlined into
 *     this implementation
 *     - fileio_connect replacing connect
 *     - fileio_read replacing read
 *     - fileio_write replacing write
 *     - fileio_close replacing close
 *
 * Author: Tatu Ylonen <ylo@cs.hut.fi>
 * Copyright (C) 1995 Tatu Ylonen <ylo@cs.hut.fi>, Espoo, Finland
 *                    All rights reserved
 * Functions for connecting the local authentication agent.
 *
 * As far as I am concerned, the code I have written for this software
 * can be used freely for any purpose.  Any derived versions of this
 * software must be clearly marked as such, and if the derived work is
 * incompatible with the protocol description in the RFC file, it must be
 * called by a name other than "ssh" or "Secure Shell".
 *
 * SSH2 implementation,
 * Copyright (C) 2000 Markus Friedl.  All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE AUTHOR ``AS IS'' AND ANY EXPRESS OR
 * IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES
 * OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED.
 * IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY DIRECT, INDIRECT,
 * INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT
 * NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
 * DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
 * THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
 * (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF
 * THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 *
 * Copyright (C) 2015 Microsoft Corp.
 * All rights reserved
 *
 * Microsoft openssh win32 port
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 *
 * 1. Redistributions of source code must retain the above copyright
 * notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 * notice, this list of conditions and the following disclaimer in the
 * documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE AUTHOR ``AS IS'' AND ANY EXPRESS OR
 * IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES
 * OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED.
 * IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY DIRECT, INDIRECT,
 * INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT
 * NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
 * DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
 * THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
 * (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF
 * THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */

#define WIN32_OPENSSH_AGENT_SOCK "\\\\.\\pipe\\openssh-ssh-agent"

static int
agent_connect_openssh(LIBSSH2_AGENT *agent)
{
    int ret = LIBSSH2_ERROR_NONE;
    const char *path;
    HANDLE pipe = INVALID_HANDLE_VALUE;
    HANDLE event = NULL;

    path = agent->identity_agent_path;
    if(!path) {
        path = getenv("SSH_AUTH_SOCK");
        if(!path)
            path = WIN32_OPENSSH_AGENT_SOCK;
    }

    for(;;) {
        pipe = CreateFileA(
            path,
            GENERIC_READ | GENERIC_WRITE,
            0,
            NULL,
            OPEN_EXISTING,
            /* Non-blocking mode for agent connections is not implemented at
             * the point this was implemented. The code for Win32 OpenSSH
             * should support non-blocking IO, but the code calling it doesn't
             * support it as of yet.
             * When non-blocking IO is implemented for the surrounding code,
             * uncomment the following line to enable support within the Win32
             * OpenSSH code.
             */
            /* FILE_FLAG_OVERLAPPED | */
            SECURITY_SQOS_PRESENT |
            SECURITY_IDENTIFICATION,
            NULL
        );

        if(pipe != INVALID_HANDLE_VALUE)
            break;
        if(GetLastError() != ERROR_PIPE_BUSY)
            break;

        /* Wait up to 1 second for a pipe instance to become available */
        if(!WaitNamedPipeA(path, 1000))
            break;
    }

    if(pipe == INVALID_HANDLE_VALUE) {
        ret = _libssh2_error(agent->session, LIBSSH2_ERROR_AGENT_PROTOCOL,
                             "unable to connect to agent pipe");
        goto cleanup;
    }

    if(SetHandleInformation(pipe, HANDLE_FLAG_INHERIT, 0) == FALSE) {
        ret = _libssh2_error(agent->session, LIBSSH2_ERROR_AGENT_PROTOCOL,
                             "unable to set handle information of agent pipe");
        goto cleanup;
    }

    event = CreateEventA(NULL, TRUE, FALSE, NULL);
    if(!event) {
        ret = _libssh2_error(agent->session, LIBSSH2_ERROR_AGENT_PROTOCOL,
                             "unable to create async I/O event");
        goto cleanup;
    }

    agent->pipe = pipe;
    pipe = INVALID_HANDLE_VALUE;
    agent->overlapped.hEvent = event;
    event = NULL;
    agent->fd = 0; /* Mark as the connection has been established */

cleanup:
    if(event)
        CloseHandle(event);
    if(pipe != INVALID_HANDLE_VALUE)
        CloseHandle(pipe);
    return ret;
}

#define RECV_SEND_ALL(func, agent, buffer, length, total)              \
    DWORD bytes_transfered;                                            \
    BOOL ret;                                                          \
    DWORD err;                                                         \
    int rc;                                                            \
                                                                       \
    while(*total < length) {                                           \
        if(!agent->pending_io)                                         \
            ret = func(agent->pipe, (char *)buffer + *total,           \
                       (DWORD)(length - *total), &bytes_transfered,    \
                       &agent->overlapped);                            \
        else                                                           \
            ret = GetOverlappedResult(agent->pipe, &agent->overlapped, \
                                      &bytes_transfered, FALSE);       \
                                                                       \
        *total += bytes_transfered;                                    \
        if(!ret) {                                                     \
            err = GetLastError();                                      \
            if((!agent->pending_io && ERROR_IO_PENDING == err)         \
               || (agent->pending_io && ERROR_IO_INCOMPLETE == err)) { \
                agent->pending_io = TRUE;                              \
                return LIBSSH2_ERROR_EAGAIN;                           \
            }                                                          \
                                                                       \
            return LIBSSH2_ERROR_SOCKET_NONE;                          \
        }                                                              \
        agent->pending_io = FALSE;                                     \
    }                                                                  \
                                                                       \
    rc = (int)*total;                                                  \
    *total = 0;                                                        \
    return rc;

static int
win32_openssh_send_all(LIBSSH2_AGENT *agent, void *buffer, size_t length,
                       size_t *send_recv_total)
{
    RECV_SEND_ALL(WriteFile, agent, buffer, length, send_recv_total)
}

static int
win32_openssh_recv_all(LIBSSH2_AGENT *agent, void *buffer, size_t length,
                       size_t *send_recv_total)
{
    RECV_SEND_ALL(ReadFile, agent, buffer, length, send_recv_total)
}

#undef RECV_SEND_ALL

static int
agent_transact_openssh(LIBSSH2_AGENT *agent, agent_transaction_ctx_t transctx)
{
    unsigned char buf[4];
    int rc;

    /* Send the length of the request */
    if(transctx->state == agent_NB_state_request_created) {
        _libssh2_htonu32(buf, (uint32_t)transctx->request_len);
        rc = win32_openssh_send_all(agent, buf, sizeof(buf),
                                    &transctx->send_recv_total);
        if(rc == LIBSSH2_ERROR_EAGAIN)
            return LIBSSH2_ERROR_EAGAIN;
        else if(rc < 0)
            return _libssh2_error(agent->session, LIBSSH2_ERROR_SOCKET_SEND,
                                  "agent send failed");
        transctx->state = agent_NB_state_request_length_sent;
    }

    /* Send the request body */
    if(transctx->state == agent_NB_state_request_length_sent) {
        rc = win32_openssh_send_all(agent, transctx->request,
                                    transctx->request_len,
                                    &transctx->send_recv_total);
        if(rc == LIBSSH2_ERROR_EAGAIN)
            return LIBSSH2_ERROR_EAGAIN;
        else if(rc < 0)
            return _libssh2_error(agent->session, LIBSSH2_ERROR_SOCKET_SEND,
                                  "agent send failed");
        transctx->state = agent_NB_state_request_sent;
    }

    /* Receive the length of the body */
    if(transctx->state == agent_NB_state_request_sent) {
        rc = win32_openssh_recv_all(agent, buf, sizeof(buf),
                                    &transctx->send_recv_total);
        if(rc == LIBSSH2_ERROR_EAGAIN)
            return LIBSSH2_ERROR_EAGAIN;
        else if(rc < 0)
            return _libssh2_error(agent->session, LIBSSH2_ERROR_SOCKET_RECV,
                                  "agent recv failed");

        transctx->response_len = _libssh2_ntohu32(buf);
        transctx->response = LIBSSH2_ALLOC(agent->session,
                                           transctx->response_len);
        if(!transctx->response)
            return LIBSSH2_ERROR_ALLOC;

        transctx->state = agent_NB_state_response_length_received;
    }

    /* Receive the response body */
    if(transctx->state == agent_NB_state_response_length_received) {
        rc = win32_openssh_recv_all(agent, transctx->response,
                                    transctx->response_len,
                                    &transctx->send_recv_total);
        if(rc == LIBSSH2_ERROR_EAGAIN)
            return LIBSSH2_ERROR_EAGAIN;
        else if(rc < 0)
            return _libssh2_error(agent->session, LIBSSH2_ERROR_SOCKET_RECV,
                                  "agent recv failed");
        transctx->state = agent_NB_state_response_received;
    }

    return LIBSSH2_ERROR_NONE;
}

static int
agent_disconnect_openssh(LIBSSH2_AGENT *agent)
{
    if(!CancelIo(agent->pipe))
        return _libssh2_error(agent->session, LIBSSH2_ERROR_SOCKET_DISCONNECT,
                              "failed to cancel pending IO of agent pipe");
    if(!CloseHandle(agent->overlapped.hEvent))
        return _libssh2_error(agent->session, LIBSSH2_ERROR_SOCKET_DISCONNECT,
                              "failed to close handle to async I/O event");
    agent->overlapped.hEvent = NULL;
    /* let queued APCs (if any) drain */
    SleepEx(0, TRUE);
    if(!CloseHandle(agent->pipe))
        return _libssh2_error(agent->session, LIBSSH2_ERROR_SOCKET_DISCONNECT,
                              "failed to close handle to agent pipe");

    agent->pipe = INVALID_HANDLE_VALUE;
    agent->fd = LIBSSH2_INVALID_SOCKET;

    return LIBSSH2_ERROR_NONE;
}

static struct agent_ops agent_ops_openssh = {
    agent_connect_openssh,
    agent_transact_openssh,
    agent_disconnect_openssh
};

#endif /* HAVE_WIN32_AGENTS */
