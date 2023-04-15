// Code generated by protoc-gen-go-grpc. DO NOT EDIT.
// versions:
// - protoc-gen-go-grpc v1.2.0
// - protoc             v3.21.12
// source: pb/spotify-remote.proto

package pb

import (
	context "context"
	grpc "google.golang.org/grpc"
	codes "google.golang.org/grpc/codes"
	status "google.golang.org/grpc/status"
)

// This is a compile-time assertion to ensure that this generated file
// is compatible with the grpc package it is being compiled against.
// Requires gRPC-Go v1.32.0 or later.
const _ = grpc.SupportPackageIsVersion7

// SpotifyRemoteClient is the client API for SpotifyRemote service.
//
// For semantics around ctx use and closing/ending streaming RPCs, please refer to https://pkg.go.dev/google.golang.org/grpc/?tab=doc#ClientConn.NewStream.
type SpotifyRemoteClient interface {
	SendAudio(ctx context.Context, opts ...grpc.CallOption) (SpotifyRemote_SendAudioClient, error)
}

type spotifyRemoteClient struct {
	cc grpc.ClientConnInterface
}

func NewSpotifyRemoteClient(cc grpc.ClientConnInterface) SpotifyRemoteClient {
	return &spotifyRemoteClient{cc}
}

func (c *spotifyRemoteClient) SendAudio(ctx context.Context, opts ...grpc.CallOption) (SpotifyRemote_SendAudioClient, error) {
	stream, err := c.cc.NewStream(ctx, &SpotifyRemote_ServiceDesc.Streams[0], "/protocol.SpotifyRemote/SendAudio", opts...)
	if err != nil {
		return nil, err
	}
	x := &spotifyRemoteSendAudioClient{stream}
	return x, nil
}

type SpotifyRemote_SendAudioClient interface {
	Send(*AudioChunk) error
	CloseAndRecv() (*SendAudioResponse, error)
	grpc.ClientStream
}

type spotifyRemoteSendAudioClient struct {
	grpc.ClientStream
}

func (x *spotifyRemoteSendAudioClient) Send(m *AudioChunk) error {
	return x.ClientStream.SendMsg(m)
}

func (x *spotifyRemoteSendAudioClient) CloseAndRecv() (*SendAudioResponse, error) {
	if err := x.ClientStream.CloseSend(); err != nil {
		return nil, err
	}
	m := new(SendAudioResponse)
	if err := x.ClientStream.RecvMsg(m); err != nil {
		return nil, err
	}
	return m, nil
}

// SpotifyRemoteServer is the server API for SpotifyRemote service.
// All implementations must embed UnimplementedSpotifyRemoteServer
// for forward compatibility
type SpotifyRemoteServer interface {
	SendAudio(SpotifyRemote_SendAudioServer) error
	mustEmbedUnimplementedSpotifyRemoteServer()
}

// UnimplementedSpotifyRemoteServer must be embedded to have forward compatible implementations.
type UnimplementedSpotifyRemoteServer struct {
}

func (UnimplementedSpotifyRemoteServer) SendAudio(SpotifyRemote_SendAudioServer) error {
	return status.Errorf(codes.Unimplemented, "method SendAudio not implemented")
}
func (UnimplementedSpotifyRemoteServer) mustEmbedUnimplementedSpotifyRemoteServer() {}

// UnsafeSpotifyRemoteServer may be embedded to opt out of forward compatibility for this service.
// Use of this interface is not recommended, as added methods to SpotifyRemoteServer will
// result in compilation errors.
type UnsafeSpotifyRemoteServer interface {
	mustEmbedUnimplementedSpotifyRemoteServer()
}

func RegisterSpotifyRemoteServer(s grpc.ServiceRegistrar, srv SpotifyRemoteServer) {
	s.RegisterService(&SpotifyRemote_ServiceDesc, srv)
}

func _SpotifyRemote_SendAudio_Handler(srv interface{}, stream grpc.ServerStream) error {
	return srv.(SpotifyRemoteServer).SendAudio(&spotifyRemoteSendAudioServer{stream})
}

type SpotifyRemote_SendAudioServer interface {
	SendAndClose(*SendAudioResponse) error
	Recv() (*AudioChunk, error)
	grpc.ServerStream
}

type spotifyRemoteSendAudioServer struct {
	grpc.ServerStream
}

func (x *spotifyRemoteSendAudioServer) SendAndClose(m *SendAudioResponse) error {
	return x.ServerStream.SendMsg(m)
}

func (x *spotifyRemoteSendAudioServer) Recv() (*AudioChunk, error) {
	m := new(AudioChunk)
	if err := x.ServerStream.RecvMsg(m); err != nil {
		return nil, err
	}
	return m, nil
}

// SpotifyRemote_ServiceDesc is the grpc.ServiceDesc for SpotifyRemote service.
// It's only intended for direct use with grpc.RegisterService,
// and not to be introspected or modified (even as a copy)
var SpotifyRemote_ServiceDesc = grpc.ServiceDesc{
	ServiceName: "protocol.SpotifyRemote",
	HandlerType: (*SpotifyRemoteServer)(nil),
	Methods:     []grpc.MethodDesc{},
	Streams: []grpc.StreamDesc{
		{
			StreamName:    "SendAudio",
			Handler:       _SpotifyRemote_SendAudio_Handler,
			ClientStreams: true,
		},
	},
	Metadata: "pb/spotify-remote.proto",
}
