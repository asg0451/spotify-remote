srcs = $(shell find . -name '*.rs' -or -name 'Cargo.toml' -or -name 'Cargo.lock' -or -name 'Dockerfile' -or -name 'Makefile' -or -name '*.proto')

.DEFAULT_GOAL := run

.PHONY: docker docker-push docker-run windows-remote

windows-remote: target/x86_64-pc-windows-gnu/release/forwarder.exe
target/x86_64-pc-windows-gnu/release/forwarder.exe: $(srcs)
	cross build --release --target x86_64-pc-windows-gnu --bin forwarder

docker-push: docker
	docker push 413471642455.dkr.ecr.us-east-1.amazonaws.com/spotify-remote-receiver:latest

docker: $(srcs) Dockerfile
	DOCKER_BUILDKIT=1 docker build . -t 413471642455.dkr.ecr.us-east-1.amazonaws.com/spotify-remote-receiver:latest

docker-run: docker
	docker run -it --rm -e RUST_BACKTRACE=full -e RUST_LOG=spotify-remote-receiver=trace,info --env-file receiver/.env 413471642455.dkr.ecr.us-east-1.amazonaws.com/spotify-remote-receiver:latest

refresh-ecr:
	ssh hex2 bash --login -c "~/kube/k3s/apps/ecr.sh"
restart:
	ssh hex2 "~/bin/kubectl -n apps rollout restart deployment/spotify-remote-receiver"

deploy: docker-push
	$(MAKE) refresh-ecr
	$(MAKE) restart

docker-creds-refresh:
	aws ecr get-login-password --region us-east-1 | docker login --username AWS --password-stdin 413471642455.dkr.ecr.us-east-1.amazonaws.com

deploy-multiarch: docker-multiarch
	$(MAKE) refresh-ecr
	$(MAKE) restart
	$(MAKE) docker-arm-builder-down

docker-multiarch: Dockerfile $(srcs) docker-arm-ensure-builder-up docker-creds-refresh
	docker buildx build --builder multiarch --platform linux/arm64,linux/amd64 --push -t 413471642455.dkr.ecr.us-east-1.amazonaws.com/spotify-remote-receiver:latest --ssh default .

docker-arm-ensure-builder-up:
	aws ec2 describe-instances --instance-ids i-00ab99709da8e22aa | jq -r '.Reservations[].Instances[].State.Name == "stopped"' | grep false || $(MAKE) docker-arm-builder-up
docker-arm-builder-up:
	aws ec2 start-instances --instance-ids i-00ab99709da8e22aa
	aws ec2 wait instance-running --instance-ids i-00ab99709da8e22aa
docker-arm-builder-down:
	aws ec2 stop-instances --instance-ids i-00ab99709da8e22aa

stop:
	ssh hex2 'PATH=$$PATH:~/bin kubectl -n apps scale --replicas 0 deployment/spotify-remote-receiver'
start:
	ssh hex2 'export PATH=$$PATH:~/bin && ./kube/k3s/apps/ecr.sh && kubectl -n apps scale --replicas 1 deployment/spotify-remote-receiver'

# setup only
docker-create-builder:
	docker buildx create --bootstrap --name multiarch --node local --platform linux/amd64
	docker buildx create --bootstrap --name multiarch --append --node aws-ec2-arm --platform linux/arm64 ssh://docker-builder-1
