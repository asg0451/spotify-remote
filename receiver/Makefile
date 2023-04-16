srcs = $(shell find . -name '*.go' | grep -v '.pb.go')
proto_srcs = $(shell find src -name '*.proto')

.PHONY: all build pb install-pb-tools

all: build

build: $(srcs) pb
	go mod tidy
	mkdir -p bin
	go build -o bin/ ./...

pb: $(proto_srcs)
	protoc -I=src --go_out=src --go-grpc_out=src --go_opt=paths=source_relative --go-grpc_opt=paths=source_relative $(proto_srcs)

grpcurl:
	which grpcurl || go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest
	grpcurl -plaintext  -d '{"input": "yoo"}' :8080 donnerator.Donnerator.Generate

install-pb-tools:
	go install google.golang.org/protobuf/cmd/protoc-gen-go@latest
	go install google.golang.org/grpc/cmd/protoc-gen-go-grpc@v1.2

restart:
	ssh hex2 "~/bin/kubectl -n apps rollout restart deployment/donnerator"
docker-push: docker docker-creds-refresh
	docker push 413471642455.dkr.ecr.us-east-1.amazonaws.com/donnerator:latest
docker: $(srcs) dict.txt
	DOCKER_BUILDKIT=1 docker build . -t 413471642455.dkr.ecr.us-east-1.amazonaws.com/donnerator:latest
deploy: docker-push
	$(MAKE) restart
docker-creds-refresh:
	aws ecr get-login-password --region us-east-1 | docker login --username AWS --password-stdin 413471642455.dkr.ecr.us-east-1.amazonaws.com

deploy-multiarch: docker-multiarch
	$(MAKE) restart
	$(MAKE) docker-arm-builder-down

docker-multiarch: $(srcs) docker-arm-ensure-builder-up docker-creds-refresh
	docker buildx build --builder multiarch --platform linux/arm64,linux/amd64 --push -t 413471642455.dkr.ecr.us-east-1.amazonaws.com/donnerator:latest .

# setup only
docker-create-builder:
	docker buildx create --bootstrap --name multiarch --node local
	docker buildx create --bootstrap --name multiarch --append --node aws-ec2-arm ssh://docker-builder-1

docker-arm-ensure-builder-up:
	aws ec2 describe-instances --instance-ids i-00ab99709da8e22aa | jq -r '.Reservations[].Instances[].State.Name == "stopped"' | grep false || $(MAKE) docker-arm-builder-up
docker-arm-builder-up:
	aws ec2 start-instances --instance-ids i-00ab99709da8e22aa
	aws ec2 wait instance-running --instance-ids i-00ab99709da8e22aa
docker-arm-builder-down:
	aws ec2 stop-instances --instance-ids i-00ab99709da8e22aa
