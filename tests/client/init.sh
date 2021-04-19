#! /bin/sh

set -e

if ! id tc 2>/dev/null ; then
    adduser -D tc
    passwd -u tc
    ssh_dir=~tc/.ssh
    mkdir -m 0700 $ssh_dir
    key_file=$ssh_dir/authorized_keys
    echo "$POSTGREAD_TEST_CLIENT_SSH_PUB_KEY_CONTENT" > $key_file
    ssh-keygen -l -f $key_file
    chmod 0600 $key_file
    chown -R tc: $ssh_dir
fi

ssh-keygen -A

exec /usr/sbin/sshd "$@"
