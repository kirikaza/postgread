image := postgread/test-client
version := 2
tag := $(image):$(version)

build:
	docker build -t $(tag) .

push: build
	docker push $(tag)