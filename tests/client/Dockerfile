FROM alpine:latest
RUN apk add --no-cache lsof openssh-server postgresql-client \
 && rm -rf /tmp/* /var/cache/apk/* /etc/ssh/ssh_host_*_key
ADD sshd_config /etc/ssh/sshd_config
ADD init.sh /init.sh
EXPOSE 22
ENTRYPOINT ["/init.sh"]
CMD ["-D", "-e"]
